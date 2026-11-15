//! Everything that is checked before the first mutation. Read only: a rehearsal is this and
//! nothing else, and an execution starts with it.

use std::fs;

use ads_core::gate::{
    AbsPath, ActionPlan, Decision, Estimate, ExecutionPermit, GatePolicy, Operation,
    OperationRequest, PlanRequest, Rule, TargetRequest, classify,
};

use crate::fence::{Fence, Kind, same_names};
use crate::limits::Limits;
use crate::manifest::{
    ITEMS_DIR, MANIFEST_NAME, MANIFEST_VERSION, Manifest, ManifestItem, folder_name,
};
use crate::reason::{Reason, Stop};
use crate::report::{Effect, Status, Step};
use crate::survey::{Facts, names, survey};

/// One mutation of the plan, on names below the fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Op {
    pub rel: Vec<String>,
    /// What the entry was when it was checked; it has to be the same right before the mutation.
    pub kind: Kind,
    pub act: Act,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Act {
    /// Move to `to`, making the missing folders between `to[..floor]` and its parent.
    /// `taken` is the refusal when something already is at `to`.
    MoveTo {
        to: Vec<String>,
        floor: usize,
        taken: Reason,
    },
    Remove {
        recursive: bool,
    },
}

/// A plan that passed every check, ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    /// Every mutation stays strictly below this folder: the work root, or its quarantine when
    /// the job empties it.
    pub floor: Vec<String>,
    /// Folders to make if they are missing, in order.
    pub ensure: Vec<Vec<String>>,
    /// Folders that must not exist and are made exclusively, in order.
    pub reserve: Vec<Vec<String>>,
    /// The manifest and where it goes; `started_at` is filled when the execution starts.
    pub manifest: Option<(Vec<String>, Manifest)>,
    pub steps: Vec<Step>,
    /// The mutation of each step; `None` for a step with nothing to do.
    pub ops: Vec<Option<Op>>,
}

struct Target {
    text: String,
    rel: Vec<String>,
    kind: Kind,
    facts: Facts,
    declared_repo_root: bool,
}

fn join(base: &[String], more: &[String]) -> Vec<String> {
    let mut joined = base.to_vec();
    joined.extend_from_slice(more);
    joined
}

fn child(base: &[String], name: &str) -> Vec<String> {
    let mut joined = base.to_vec();
    joined.push(name.to_owned());
    joined
}

fn starts_with(path: &[String], base: &[String]) -> bool {
    path.len() >= base.len() && same_names(&path[..base.len()], base)
}

fn same_path(a: &AbsPath, b: &AbsPath) -> bool {
    a.is_within(b) && b.is_within(a)
}

/// The existing folders from `path[..floor]` down to `path[..until]` have to be plain folders;
/// the ones that are missing are returned, in the order to make them.
fn missing_folders(
    fence: &Fence,
    path: &[String],
    floor: usize,
    until: usize,
) -> Result<Vec<Vec<String>>, Stop> {
    for depth in floor + 1..=until {
        let rel = &path[..depth];
        match fence.inspect(rel)? {
            (_, Some(Kind::Dir)) => {}
            (_, None) => return Ok((depth..=until).map(|d| path[..d].to_vec()).collect()),
            (found, Some(Kind::File)) => {
                return Err(Stop::new(
                    Reason::NotADirectory,
                    format!("{} is not a folder", found.display()),
                ));
            }
            (found, Some(Kind::Link { .. })) => {
                return Err(Stop::new(
                    Reason::LinkInPath,
                    format!("{} is a link", found.display()),
                ));
            }
        }
    }
    Ok(Vec::new())
}

pub(crate) fn prepare(
    fence: &Fence,
    policy: &GatePolicy,
    limits: &Limits,
    permit: &ExecutionPermit,
) -> Result<Job, Stop> {
    let plan = permit.plan();

    // The permit proves the plan passed a gate; this is the gate of the trusted policy again.
    let declared = classify(plan, policy);
    if declared.decision() == Decision::Denied {
        let codes: Vec<String> = declared
            .findings()
            .iter()
            .map(|finding| finding.rule.code().to_string())
            .collect();
        return Err(Stop::new(Reason::PolicyDenies, codes.join(", ")));
    }
    if declared.is_quarantined() {
        return Err(Stop::new(
            Reason::QuarantineBypassed,
            "the policy rewrites this delete as a move to quarantine",
        ));
    }
    if declared.decision() == Decision::RequiresApproval && permit.approval_id().is_none() {
        return Err(Stop::new(
            Reason::ApprovalMissing,
            "the policy asks for an approval",
        ));
    }

    let work_root = plan.work_root();
    let work = fence.relative(work_root)?;
    match fence.existing(&work)? {
        (_, Kind::Dir) => {}
        (path, Kind::File) => {
            return Err(Stop::new(
                Reason::NotADirectory,
                format!("the work root {} is not a folder", path.display()),
            ));
        }
        (path, Kind::Link { .. }) => {
            return Err(Stop::new(
                Reason::LinkInPath,
                format!("the work root {} is a link", path.display()),
            ));
        }
    }

    let mut targets = Vec::new();
    for target in plan.targets() {
        let scope = target.path().scope();
        if target.path().pattern().is_some() {
            return Err(Stop::new(
                Reason::Unsupported,
                format!("{} is a wildcard target", target.path()),
            ));
        }
        let rel = fence.relative(scope)?;
        if !scope.is_strictly_within(work_root)
            || rel.len() <= work.len()
            || !starts_with(&rel, &work)
        {
            return Err(Stop::new(
                Reason::OutsideWorkRoot,
                format!("{scope} is not strictly inside the work root"),
            ));
        }
        let (kind, facts) = survey(fence, &rel, limits.max_depth)?;
        targets.push(Target {
            text: scope.to_string(),
            rel,
            kind,
            facts,
            declared_repo_root: target.is_repo_root(),
        });
    }
    for (at, one) in targets.iter().enumerate() {
        for other in &targets[at + 1..] {
            if starts_with(&one.rel, &other.rel) || starts_with(&other.rel, &one.rel) {
                return Err(Stop::new(
                    Reason::OverlappingTargets,
                    format!("{} and {}", one.text, other.text),
                ));
            }
        }
    }

    check_facts(policy, permit, declared.decision(), &targets)?;

    let quarantine = policy.quarantine_root(work_root);
    let job = build_job(fence, policy, limits, permit, work, &targets, &quarantine)?;

    // The verdict judged the folder that receives the targets. Where each one really lands
    // is judged here: by text against the policy and the work root, and on disk by every
    // mutation (see `disk`).
    for op in job.ops.iter().flatten() {
        let Act::MoveTo { to, .. } = &op.act else {
            continue;
        };
        let text = fence.text_of(to);
        let landing =
            AbsPath::parse(&text).map_err(|error| Stop::new(Reason::NotPhysical, error))?;
        if !landing.is_strictly_within(work_root) {
            return Err(Stop::new(
                Reason::OutsideWorkRoot,
                format!("{text} is not strictly inside the work root"),
            ));
        }
        if let Some(protection) = policy.protection(&landing) {
            return Err(Stop::new(
                Reason::PolicyDenies,
                format!("{text} is protected ({})", protection.code()),
            ));
        }
    }
    Ok(job)
}

fn build_job(
    fence: &Fence,
    policy: &GatePolicy,
    limits: &Limits,
    permit: &ExecutionPermit,
    work: Vec<String>,
    targets: &[Target],
    quarantine: &AbsPath,
) -> Result<Job, Stop> {
    let plan = permit.plan();
    let work_root = plan.work_root();
    let declared = classify(plan, policy);
    match plan.operation() {
        Operation::Overwrite | Operation::CleanUntracked => Err(Stop::new(
            Reason::Unsupported,
            format!("{} is not executed here", plan.operation().name().as_str()),
        )),
        Operation::Delete | Operation::DeleteRecursive => {
            // A real delete exists for two cases only: emptying the quarantine, with the purge
            // rule fired and approved, and a policy that turned the quarantine off.
            if policy.quarantine_enabled() {
                let purge = declared.has(Rule::QuarantinePurge)
                    && permit.approval_id().is_some()
                    && plan
                        .targets()
                        .iter()
                        .all(|target| target.path().scope().is_strictly_within(quarantine));
                if !purge {
                    return Err(Stop::new(
                        Reason::QuarantineBypassed,
                        "a delete outside the quarantine, with the quarantine on",
                    ));
                }
            }
            let recursive = matches!(plan.operation(), Operation::DeleteRecursive);
            let floor = if policy.quarantine_enabled() {
                fence.relative(quarantine)?
            } else {
                work
            };
            delete_job(fence, floor, targets, recursive)
        }
        Operation::Move { destination } => {
            let into_quarantine =
                policy.quarantine_enabled() && destination.is_strictly_within(quarantine);
            if into_quarantine {
                quarantine_job(fence, permit, work, targets, destination, quarantine)
            } else if same_path(destination, work_root) {
                if !policy.quarantine_enabled() {
                    return Err(Stop::new(Reason::Unsupported, "the quarantine is off"));
                }
                restore_job(fence, limits, permit, work, targets, quarantine)
            } else {
                Err(Stop::new(
                    Reason::Unsupported,
                    "a move that is neither into the quarantine nor a restore",
                ))
            }
        }
    }
}

/// Judges the plan again with what is on disk in place of what was declared. A stricter
/// answer, or more than what an approval was given for, refuses the execution.
fn check_facts(
    policy: &GatePolicy,
    permit: &ExecutionPermit,
    declared: Decision,
    targets: &[Target],
) -> Result<(), Stop> {
    let plan = permit.plan();
    let real = Estimate {
        items: targets.iter().map(|target| target.facts.items).sum(),
        bytes: targets.iter().map(|target| target.facts.bytes).sum(),
    };
    let request = PlanRequest {
        operation: match plan.operation() {
            Operation::Delete => OperationRequest::Delete,
            Operation::DeleteRecursive => OperationRequest::DeleteRecursive,
            Operation::Overwrite => OperationRequest::Overwrite,
            Operation::CleanUntracked => OperationRequest::CleanUntracked,
            Operation::Move { destination } => OperationRequest::Move {
                destination: destination.to_string(),
            },
        },
        targets: targets
            .iter()
            .map(|target| TargetRequest {
                path: target.text.clone(),
                repo_root: target.declared_repo_root || target.facts.repo_root,
            })
            .collect(),
        work_root: plan.work_root().to_string(),
        estimate: Some(real),
    };
    let real_plan = ActionPlan::normalize(&request)
        .map_err(|error| Stop::new(Reason::FactsContradictPlan, error))?;
    let judged = classify(&real_plan, policy);
    if judged.decision() > declared {
        let codes: Vec<String> = judged
            .findings()
            .iter()
            .map(|finding| finding.rule.code().to_string())
            .collect();
        return Err(Stop::new(
            Reason::FactsContradictPlan,
            format!(
                "on disk: {} items, {} bytes; rules: {}",
                real.items,
                real.bytes,
                codes.join(", ")
            ),
        ));
    }
    if let (Some(_), Some(estimate)) = (permit.approval_id(), plan.estimate())
        && (real.items > estimate.items || real.bytes > estimate.bytes)
    {
        return Err(Stop::new(
            Reason::FactsContradictPlan,
            format!(
                "approved for {} items and {} bytes; on disk: {} items, {} bytes",
                estimate.items, estimate.bytes, real.items, real.bytes
            ),
        ));
    }
    Ok(())
}

fn delete_job(
    fence: &Fence,
    work: Vec<String>,
    targets: &[Target],
    recursive: bool,
) -> Result<Job, Stop> {
    let mut steps = Vec::new();
    let mut ops = Vec::new();
    for target in targets {
        if !recursive && target.kind == Kind::Dir {
            let (path, _) = fence.existing(&target.rel)?;
            if !names(&path)?.is_empty() {
                return Err(Stop::new(
                    Reason::NotEmpty,
                    format!("{} is not empty", target.text),
                ));
            }
        }
        steps.push(Step {
            target: target.text.clone(),
            effect: if recursive {
                Effect::DeleteRecursive
            } else {
                Effect::Delete
            },
            facts: target.facts,
            status: Status::Pending,
        });
        ops.push(Some(Op {
            rel: target.rel.clone(),
            kind: target.kind,
            act: Act::Remove { recursive },
        }));
    }
    Ok(Job {
        floor: work,
        ensure: Vec::new(),
        reserve: Vec::new(),
        manifest: None,
        steps,
        ops,
    })
}

fn quarantine_job(
    fence: &Fence,
    permit: &ExecutionPermit,
    work: Vec<String>,
    targets: &[Target],
    destination: &AbsPath,
    quarantine: &AbsPath,
) -> Result<Job, Stop> {
    let plan = permit.plan();
    if let Some(inside) = plan
        .targets()
        .iter()
        .find(|target| target.path().scope().is_within(quarantine))
    {
        return Err(Stop::new(
            Reason::Unsupported,
            format!("{} is already in quarantine", inside.path()),
        ));
    }
    let name = folder_name(permit.action_id())?;
    let dest = fence.relative(destination)?;
    if dest.len() <= work.len() || !starts_with(&dest, &work) {
        return Err(Stop::new(
            Reason::OutsideWorkRoot,
            format!("{destination} is not strictly inside the work root"),
        ));
    }
    let ensure = missing_folders(fence, &dest, work.len(), dest.len())?;
    let batch = child(&dest, &name);
    if ensure.is_empty() && fence.inspect(&batch)?.1.is_some() {
        return Err(Stop::new(
            Reason::DestinationTaken,
            format!("{} already exists", fence.text_of(&batch)),
        ));
    }
    let items = child(&batch, ITEMS_DIR);

    let mut steps = Vec::new();
    let mut ops = Vec::new();
    let mut listed = Vec::new();
    for target in targets {
        let below_work = &target.rel[work.len()..];
        let stored = join(&items, below_work);
        listed.push(ManifestItem {
            original: target.text.clone(),
            path: below_work.to_vec(),
            kind: target.kind.into(),
            items: target.facts.items,
            bytes: target.facts.bytes,
        });
        steps.push(Step {
            target: target.text.clone(),
            effect: Effect::Quarantine {
                stored: fence.text_of(&stored),
            },
            facts: target.facts,
            status: Status::Pending,
        });
        ops.push(Some(Op {
            rel: target.rel.clone(),
            kind: target.kind,
            act: Act::MoveTo {
                to: stored,
                floor: items.len(),
                taken: Reason::DestinationTaken,
            },
        }));
    }
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        action_id: name,
        plan_hash: permit.plan_hash().to_string(),
        started_at: String::new(),
        work_root: plan.work_root().to_string(),
        items: listed,
    };
    Ok(Job {
        floor: work,
        ensure,
        reserve: vec![batch.clone(), items],
        manifest: Some((child(&batch, MANIFEST_NAME), manifest)),
        steps,
        ops,
    })
}

/// A restore is a move of one quarantine batch back to the work root: every item of its
/// manifest that is still in quarantine goes back to where it came from.
fn restore_job(
    fence: &Fence,
    limits: &Limits,
    permit: &ExecutionPermit,
    work: Vec<String>,
    targets: &[Target],
    quarantine: &AbsPath,
) -> Result<Job, Stop> {
    let plan = permit.plan();
    let [batch] = targets else {
        return Err(Stop::new(
            Reason::Unsupported,
            "a restore takes one quarantine batch",
        ));
    };
    let in_quarantine = plan.targets()[0]
        .path()
        .scope()
        .is_strictly_within(quarantine);
    if !in_quarantine || batch.kind != Kind::Dir {
        return Err(Stop::new(
            Reason::Unsupported,
            format!("{} is not a quarantine batch", batch.text),
        ));
    }

    let invalid =
        |what: &str| Stop::new(Reason::ManifestInvalid, format!("{}: {what}", batch.text));
    let (manifest_path, kind) = match fence.inspect(&child(&batch.rel, MANIFEST_NAME))? {
        (path, Some(kind)) => (path, kind),
        (_, None) => return Err(invalid("no manifest")),
    };
    if kind != Kind::File {
        return Err(invalid("the manifest is not a file"));
    }
    let size = fs::symlink_metadata(&manifest_path)
        .map_err(|error| Stop::io("looking at", &manifest_path, &error))?
        .len();
    if size > limits.max_manifest_bytes {
        return Err(invalid("the manifest is too large"));
    }
    let bytes =
        fs::read(&manifest_path).map_err(|error| Stop::io("reading", &manifest_path, &error))?;
    let manifest = Manifest::from_bytes(&bytes)?;
    if batch.rel.last() != Some(&manifest.action_id) {
        return Err(invalid("the manifest is of another action"));
    }
    if manifest.work_root != plan.work_root().to_string() {
        return Err(invalid("the manifest is of another work root"));
    }
    for (at, one) in manifest.items.iter().enumerate() {
        for other in &manifest.items[at + 1..] {
            if starts_with(&one.path, &other.path) || starts_with(&other.path, &one.path) {
                return Err(invalid("two items share a place"));
            }
        }
    }

    let items = child(&batch.rel, ITEMS_DIR);
    let mut steps = Vec::new();
    let mut ops = Vec::new();
    for item in &manifest.items {
        let original = join(&work, &item.path);
        // The original path is rebuilt from the names; the text has to say the same.
        let written = AbsPath::parse(&item.original)
            .ok()
            .and_then(|path| fence.relative(&path).ok());
        if written.as_deref() != Some(original.as_slice()) {
            return Err(invalid("an item whose path does not match its place"));
        }
        let stored = join(&items, &item.path);
        let step = |facts: Facts, status: Status| Step {
            target: fence.text_of(&original),
            effect: Effect::Restore {
                stored: fence.text_of(&stored),
            },
            facts,
            status,
        };
        let found = match survey(fence, &stored, limits.max_depth) {
            Ok(found) => Some(found),
            Err(stop) if stop.reason == Reason::Missing => None,
            Err(stop) => return Err(stop),
        };
        let Some((kind, facts)) = found else {
            steps.push(step(Facts::default(), Status::Absent));
            ops.push(None);
            continue;
        };
        let parents = original.len() - 1;
        if missing_folders(fence, &original, work.len(), parents)?.is_empty()
            && fence.inspect(&original)?.1.is_some()
        {
            return Err(Stop::new(
                Reason::RestorePlaceTaken,
                format!("{} exists", fence.text_of(&original)),
            ));
        }
        steps.push(step(facts, Status::Pending));
        ops.push(Some(Op {
            rel: stored,
            kind,
            act: Act::MoveTo {
                to: original,
                floor: work.len(),
                taken: Reason::RestorePlaceTaken,
            },
        }));
    }
    if ops.iter().all(Option::is_none) {
        return Err(Stop::new(
            Reason::NothingToRestore,
            format!("{} holds none of its items", batch.text),
        ));
    }
    Ok(Job {
        floor: work,
        ensure: Vec::new(),
        reserve: Vec::new(),
        manifest: None,
        steps,
        ops,
    })
}
