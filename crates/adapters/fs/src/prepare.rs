//! Everything that is checked before the first mutation. Read only: a rehearsal is this and
//! nothing else, and an execution starts with it.

use ads_core::gate::{
    AbsPath, ActionPlan, Decision, Estimate, ExecutionPermit, GatePolicy, Operation,
    OperationRequest, PlanRequest, Rule, TargetRequest, classify, may_name,
};

use crate::fence::{Fence, Kind};
use crate::limits::{DefinitiveDelete, Limits};
use crate::manifest::{
    ITEMS_DIR, MANIFEST_NAME, MANIFEST_VERSION, Manifest, ManifestItem, folder_name,
};
use crate::reason::{Reason, Stop};
use crate::report::{Effect, Laid, Report, Scaffold, Staged, Status, Step};
use crate::survey::{
    Facts, Seen, charge_entries, list_with_budget, open_listed, survey, survey_with_budget, total,
};
use crate::sys;

/// One mutation of the plan, on names below the fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Op {
    pub rel: Vec<String>,
    /// The inventory the approval covers: the object under the name and, for a folder, every
    /// object in its tree. The mutation refuses anything else.
    pub seen: Seen,
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
    /// Remove for good. A tree goes through `place` (below the holding folder of the job):
    /// it is moved there first and removed there. One entry is removed where it is.
    Remove { place: Option<Vec<String>> },
}

/// A plan that passed every check, ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    /// Every mutation stays strictly below this folder: the work root, or its quarantine when
    /// the job empties it.
    pub floor: Vec<String>,
    /// What is laid down before the first target, in order; one per `report.staging`.
    pub stages: Vec<Stage>,
    /// The manifest of the batch; `started_at` is filled when the execution starts.
    pub manifest: Option<Manifest>,
    /// The holding folder of the job, made for it alone: where trees are removed for good.
    pub private: Option<Vec<String>>,
    /// How many names deep the work root is.
    pub work: usize,
    pub report: Report,
    /// The mutation of each step; `None` for a step with nothing to do.
    pub ops: Vec<Option<Op>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lay {
    /// A folder to make if it is missing.
    Ensure,
    /// A folder that must not exist and is made exclusively.
    Reserve,
    /// The same, for this executor alone as far as the system has a way to say so.
    ReservePrivate,
    /// The manifest of the job, written as a new file.
    Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stage {
    pub rel: Vec<String>,
    pub lay: Lay,
}

impl Job {
    fn new(floor: Vec<String>, steps: Vec<Step>, ops: Vec<Option<Op>>) -> Self {
        Self {
            floor,
            stages: Vec::new(),
            manifest: None,
            private: None,
            work: 0,
            report: Report {
                staging: Vec::new(),
                steps,
            },
            ops,
        }
    }

    fn lay(&mut self, fence: &Fence, rel: Vec<String>, lay: Lay) {
        self.report.staging.push(Staged {
            path: fence.text_of(&rel),
            what: match lay {
                Lay::Ensure | Lay::Reserve | Lay::ReservePrivate => Scaffold::Folder,
                Lay::Manifest => Scaffold::Manifest,
            },
            status: Laid::Pending,
        });
        self.stages.push(Stage { rel, lay });
    }

    /// The manifest as it goes to disk. Larger than the executor reads back is refused
    /// before anything is changed: a batch that cannot be restored is not made.
    pub(crate) fn manifest_bytes(&self, limits: &Limits) -> Result<Option<Vec<u8>>, Stop> {
        let Some(manifest) = &self.manifest else {
            return Ok(None);
        };
        let bytes = manifest.to_bytes();
        if bytes.len() as u64 > limits.max_manifest_bytes {
            return Err(Stop::new(
                Reason::ManifestTooLarge,
                format!(
                    "the manifest would have {} bytes; the limit is {}",
                    bytes.len(),
                    limits.max_manifest_bytes
                ),
            ));
        }
        Ok(Some(bytes))
    }
}

struct Target {
    text: String,
    rel: Vec<String>,
    seen: Seen,
    facts: Facts,
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
    path.starts_with(base)
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
        match fence.locate(rel)?.entry.map(|entry| entry.kind) {
            Some(Kind::Dir) => {}
            None => return Ok((depth..=until).map(|d| path[..d].to_vec()).collect()),
            Some(Kind::File) => {
                return Err(Stop::new(
                    Reason::NotADirectory,
                    format!("{} is not a folder", fence.text_of(rel)),
                ));
            }
            Some(Kind::Link) => {
                return Err(Stop::new(
                    Reason::LinkInPath,
                    format!("{} is a link", fence.text_of(rel)),
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
    let declared =
        classify(plan, policy).map_err(|error| Stop::caused_by(Reason::PolicyDenies, error))?;
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
    // Every name down to the work root has to be a folder; a link is refused.
    fence.open_dir(&work)?;

    let targets = inspect_targets(fence, limits, plan, &work, None)?;
    check_facts(fence, policy, permit, declared.decision(), &targets)?;
    if let Some(wildcard) = plan
        .targets()
        .iter()
        .find(|target| target.path().pattern().is_some())
    {
        return Err(Stop::new(
            Reason::Unsupported,
            format!("{} is a wildcard target", wildcard.path()),
        ));
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

    let quarantine = policy.quarantine_root(work_root);
    let job = build_job(fence, policy, limits, permit, work, &targets, &quarantine)?;

    // The verdict judged the folder that receives the targets. Where each one really lands
    // is judged here: by text against the policy and the work root, and on disk by every
    // mutation (see `disk`).
    for op in job.ops.iter().flatten() {
        let to = match &op.act {
            Act::MoveTo { to, .. } => to,
            Act::Remove { place: Some(place) } => place,
            Act::Remove { place: None } => continue,
        };
        let text = fence.text_of(to);
        let landing =
            AbsPath::parse(&text).map_err(|error| Stop::caused_by(Reason::NotPhysical, error))?;
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
    let declared =
        classify(plan, policy).map_err(|error| Stop::caused_by(Reason::PolicyDenies, error))?;
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
                work.clone()
            };
            delete_job(fence, limits, permit, &work, floor, targets, recursive)
        }
        Operation::Move { destination } => {
            let into_quarantine =
                policy.quarantine_enabled() && destination.is_strictly_within(quarantine);
            if into_quarantine {
                quarantine_job(
                    fence,
                    limits,
                    permit,
                    work,
                    targets,
                    destination,
                    quarantine,
                )
            } else if same_path(destination, work_root) {
                if !policy.quarantine_enabled() {
                    return Err(Stop::new(Reason::Unsupported, "the quarantine is off"));
                }
                restore_job(fence, policy, limits, permit, work, targets, quarantine)
            } else {
                Err(Stop::new(
                    Reason::Unsupported,
                    "a move that is neither into the quarantine nor a restore",
                ))
            }
        }
    }
}

fn inspect_targets(
    fence: &Fence,
    limits: &Limits,
    plan: &ActionPlan,
    work: &[String],
    mut budget: Option<usize>,
) -> Result<Vec<Target>, Stop> {
    let mut targets = Vec::new();
    if budget.is_some() {
        fence.open_dir_with_budget(work, &mut budget)?;
    }
    for target in plan.targets() {
        let scope = target.path().scope();
        let rel = fence.relative(scope)?;
        if !scope.is_strictly_within(plan.work_root())
            || rel.len() <= work.len()
            || !starts_with(&rel, work)
        {
            return Err(Stop::new(
                Reason::OutsideWorkRoot,
                format!("{scope} is not strictly inside the work root"),
            ));
        }
        charge_entries(&mut budget, 1)?;
        if budget.is_some() && rel.len() - work.len() > limits.max_depth as usize {
            return Err(Stop::new(
                Reason::TooDeep,
                format!("{scope} exceeds max_depth"),
            ));
        }
        let mut inspect = |rel: Vec<String>,
                           entry: crate::fence::Entry,
                           budget: &mut Option<usize>| {
            let (seen, facts) = survey_with_budget(fence, &entry, &rel, limits.max_depth, budget)?;
            targets.push(Target {
                text: fence.text_of(&rel),
                rel,
                seen,
                facts,
            });
            Ok::<_, Stop>(())
        };
        if let Some(pattern) = target.path().pattern() {
            let dir = fence.open_dir_with_budget(&rel, &mut budget)?;
            for name in list_with_budget(fence, &dir, &rel, &mut budget)? {
                if may_name(pattern, &name) {
                    let path = child(&rel, &name);
                    let entry = open_listed(fence, &dir, &path, &name)?;
                    inspect(path, entry, &mut budget)?;
                }
            }
        } else {
            let (_, entry) = fence.existing_with_budget(&rel, &mut budget)?;
            inspect(rel, entry, &mut budget)?;
        }
    }
    Ok(targets)
}

fn repository_targets(
    fence: &Fence,
    rel: &mut Vec<String>,
    seen: &Seen,
    targets: &mut Vec<TargetRequest>,
) {
    let metadata = |name: &str| name.eq_ignore_ascii_case(".git");
    let repo_root = seen.children.keys().any(|name| metadata(name));
    let is_metadata = rel.last().is_some_and(|name| metadata(name));
    if repo_root || is_metadata {
        targets.push(TargetRequest {
            path: fence.text_of(rel),
            repo_root,
        });
    }
    // The repository marker itself is protected as a whole.
    if !is_metadata {
        for (name, entry) in &seen.children {
            rel.push(name.clone());
            repository_targets(fence, rel, entry, targets);
            rel.pop();
        }
    }
}

fn with_repository_targets(
    fence: &Fence,
    plan: &ActionPlan,
    inspected: &[Target],
    estimate: Option<Estimate>,
) -> Result<ActionPlan, Stop> {
    let mut targets = plan
        .targets()
        .iter()
        .map(|target| TargetRequest {
            path: target.path().to_string(),
            repo_root: target.is_repo_root(),
        })
        .collect::<Vec<_>>();
    for target in inspected {
        if target.facts.repo_root
            || target.facts.nested_repo
            || target
                .rel
                .last()
                .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
        {
            repository_targets(fence, &mut target.rel.clone(), &target.seen, &mut targets);
        }
    }
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
        targets,
        work_root: plan.work_root().to_string(),
        estimate,
    };
    ActionPlan::normalize(&request)
        .map_err(|error| Stop::caused_by(Reason::FactsContradictPlan, error))
}

/// Supplies facts for the plan that will be saved to the journal. Execution checks again.
pub(crate) fn observe_plan(
    fence: &Fence,
    policy: &GatePolicy,
    limits: &Limits,
    plan: &ActionPlan,
) -> Result<ActionPlan, Stop> {
    if classify(plan, policy)
        .map_err(|error| Stop::caused_by(Reason::PolicyDenies, error))?
        .decision()
        == Decision::Denied
    {
        return Ok(plan.clone());
    }
    let work = fence.relative(plan.work_root())?;
    let targets = inspect_targets(fence, limits, plan, &work, Some(limits.max_plan_entries))?;
    with_repository_targets(fence, plan, &targets, plan.estimate())
}

/// Judges the plan again with the facts on disk.
fn check_facts(
    fence: &Fence,
    policy: &GatePolicy,
    permit: &ExecutionPermit,
    declared: Decision,
    targets: &[Target],
) -> Result<(), Stop> {
    let plan = permit.plan();
    let (items, bytes) = total(targets.iter().map(|target| &target.facts))?;
    let real = Estimate { items, bytes };
    let real_plan = with_repository_targets(fence, plan, targets, Some(real))?;
    let judged = classify(&real_plan, policy)
        .map_err(|error| Stop::caused_by(Reason::PolicyDenies, error))?;
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

/// A target that holds a repository of its own, at any depth, is not deleted nor sent to
/// quarantine: the plan and its approval speak of one folder, not of a repository in it.
fn refuse_nested_repositories(targets: &[Target]) -> Result<(), Stop> {
    match targets.iter().find(|target| target.facts.nested_repo) {
        Some(target) => Err(Stop::new(
            Reason::NestedRepository,
            format!("{} holds a repository", target.text),
        )),
        None => Ok(()),
    }
}

fn delete_job(
    fence: &Fence,
    limits: &Limits,
    permit: &ExecutionPermit,
    work: &[String],
    floor: Vec<String>,
    targets: &[Target],
    recursive: bool,
) -> Result<Job, Stop> {
    refuse_nested_repositories(targets)?;
    // A removal acts on a name inside an open folder. Unless the system can hold that folder
    // where it is, another process can take it out of the fence between the last check and
    // the call: the strictest answer is not to remove for good there.
    match limits.definitive_delete {
        DefinitiveDelete::Refuse => {
            return Err(Stop::new(
                Reason::DeleteUncontained,
                "definitive deletes are turned off (`definitive_delete = \"refuse\"`)",
            ));
        }
        DefinitiveDelete::Contained if !sys::HOLDS => {
            return Err(Stop::new(
                Reason::DeleteUncontained,
                "this system cannot hold a folder in place while its entries are removed; \
                 only the quarantine runs here (see `definitive_delete`)",
            ));
        }
        DefinitiveDelete::Contained | DefinitiveDelete::Isolated => {}
    }

    // A tree is not emptied where it is: it goes, whole, to a folder made for this
    // execution, and is removed there.
    let tree = |target: &Target| {
        recursive && target.seen.kind == Kind::Dir && !target.seen.children.is_empty()
    };
    let private = join(work, &limits.private_dir);
    let holding = match targets.iter().any(tree) {
        true => Some(child(&private, &folder_name(permit.action_id())?)),
        false => None,
    };
    for target in targets {
        // The private folder is not a target, nor is the holding folder of this execution;
        // what another execution left in the private folder is.
        let overlaps = starts_with(&private, &target.rel)
            || holding.as_ref().is_some_and(|holding| {
                starts_with(&target.rel, holding) || starts_with(holding, &target.rel)
            });
        if overlaps {
            return Err(Stop::new(
                Reason::Unsupported,
                format!(
                    "{} overlaps the private folder of the executor",
                    target.text
                ),
            ));
        }
    }

    let mut steps = Vec::new();
    let mut ops = Vec::new();
    for (at, target) in targets.iter().enumerate() {
        if !recursive && !target.seen.children.is_empty() {
            return Err(Stop::new(
                Reason::NotEmpty,
                format!("{} is not empty", target.text),
            ));
        }
        let place = match (&holding, tree(target)) {
            (Some(holding), true) => Some(child(holding, &at.to_string())),
            _ => None,
        };
        steps.push(Step {
            target: target.text.clone(),
            effect: if recursive {
                Effect::DeleteRecursive
            } else {
                Effect::Delete
            },
            facts: target.facts,
            held: place.as_ref().map(|place| fence.text_of(place)),
            status: Status::Pending,
        });
        ops.push(Some(Op {
            rel: target.rel.clone(),
            seen: target.seen.clone(),
            act: Act::Remove { place },
        }));
    }
    let mut job = Job::new(floor, steps, ops);
    if let Some(holding) = holding {
        let ensure = missing_folders(fence, &private, work.len(), private.len())?;
        if ensure.is_empty() && fence.locate(&holding)?.entry.is_some() {
            return Err(Stop::new(
                Reason::DestinationTaken,
                format!("{} already exists", fence.text_of(&holding)),
            ));
        }
        for rel in ensure {
            job.lay(fence, rel, Lay::Ensure);
        }
        job.lay(fence, holding.clone(), Lay::ReservePrivate);
        job.private = Some(holding);
        job.work = work.len();
    }
    Ok(job)
}

/// Where a restore puts one item is judged by the gate, with its own rules for the entry a
/// move creates: not a protected folder, not on a target, and not a reserved folder (the work
/// root, an allowed root, the quarantine) nor a folder that holds one. The verdict of the
/// permit judged the work root as the destination; the manifest says the real ones.
fn judge_restore_place(
    policy: &GatePolicy,
    work_root: &AbsPath,
    stored: &str,
    folder: &str,
    place: &str,
) -> Result<(), Stop> {
    let request = PlanRequest {
        operation: OperationRequest::Move {
            destination: folder.to_owned(),
        },
        targets: vec![TargetRequest {
            path: stored.to_owned(),
            repo_root: false,
        }],
        work_root: work_root.to_string(),
        estimate: Some(Estimate { items: 1, bytes: 0 }),
    };
    let plan = ActionPlan::normalize(&request).map_err(|error| {
        Stop::new(Reason::PolicyDenies, format!("{place}: {error}")).with_cause(error)
    })?;
    let verdict =
        classify(&plan, policy).map_err(|error| Stop::caused_by(Reason::PolicyDenies, error))?;
    if verdict.decision() == Decision::Denied {
        let codes: Vec<String> = verdict
            .findings()
            .iter()
            .filter(|finding| finding.rule.decision() == Decision::Denied)
            .map(|finding| finding.rule.code().to_string())
            .collect();
        return Err(Stop::new(
            Reason::PolicyDenies,
            format!("restoring to {place}: {}", codes.join(", ")),
        ));
    }
    Ok(())
}

fn quarantine_job(
    fence: &Fence,
    limits: &Limits,
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
    refuse_nested_repositories(targets)?;
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
    if ensure.is_empty() && fence.locate(&batch)?.entry.is_some() {
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
            kind: target.seen.kind.into(),
            items: target.facts.items,
            bytes: target.facts.bytes,
        });
        steps.push(Step {
            target: target.text.clone(),
            effect: Effect::Quarantine {
                stored: fence.text_of(&stored),
            },
            facts: target.facts,
            held: None,
            status: Status::Pending,
        });
        ops.push(Some(Op {
            rel: target.rel.clone(),
            seen: target.seen.clone(),
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
    let mut job = Job::new(work, steps, ops);
    for rel in ensure {
        job.lay(fence, rel, Lay::Ensure);
    }
    job.lay(fence, batch.clone(), Lay::Reserve);
    job.lay(fence, items, Lay::Reserve);
    job.lay(fence, child(&batch, MANIFEST_NAME), Lay::Manifest);
    // The instant of the start is not known in a rehearsal: the longest one stands in for
    // the size, which is checked again with the real one before anything is changed.
    job.manifest = Some(Manifest {
        started_at: "9999-12-31T23:59:59.999999999+00:00".to_owned(),
        ..manifest.clone()
    });
    job.manifest_bytes(limits)?;
    job.manifest = Some(manifest);
    Ok(job)
}

/// A restore is a move of one quarantine batch back to the work root: every item of its
/// manifest that is still in quarantine goes back to where it came from.
fn restore_job(
    fence: &Fence,
    policy: &GatePolicy,
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
    if !in_quarantine || batch.seen.kind != Kind::Dir {
        return Err(Stop::new(
            Reason::Unsupported,
            format!("{} is not a quarantine batch", batch.text),
        ));
    }

    let invalid =
        |what: &str| Stop::new(Reason::ManifestInvalid, format!("{}: {what}", batch.text));
    // The manifest is read from the open folder of the batch, by the file that was opened:
    // a link is not followed and no more than the limit is read.
    let folder = fence.open_dir(&batch.rel)?;
    let bytes = match folder.read_file(MANIFEST_NAME, limits.max_manifest_bytes) {
        Ok(bytes) => bytes,
        Err(error @ sys::Error::Missing(_)) => return Err(invalid("no manifest").with_cause(error)),
        Err(sys::Error::Io(error)) if error.kind() == std::io::ErrorKind::FileTooLarge => {
            return Err(invalid("the manifest is too large").with_cause(error));
        }
        Err(error) => {
            return Err(invalid(&format!("the manifest is not a file ({error})")).with_cause(error));
        }
    };
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
    // By text first, for every item, before the disk is looked at for any of them: the
    // place each one goes back to is the one its path says, and the gate lets a move
    // create an entry there.
    for item in &manifest.items {
        let original = join(&work, &item.path);
        // The original path is rebuilt from the names; the text has to say the same.
        let written = AbsPath::parse(&item.original)
            .ok()
            .and_then(|path| fence.relative(&path).ok());
        if written.as_deref() != Some(original.as_slice()) {
            return Err(invalid("an item whose path does not match its place"));
        }
        judge_restore_place(
            policy,
            plan.work_root(),
            &fence.text_of(&join(&items, &item.path)),
            &fence.text_of(&original[..original.len() - 1]),
            &fence.text_of(&original),
        )?;
    }

    let mut steps = Vec::new();
    let mut ops = Vec::new();
    for item in &manifest.items {
        let original = join(&work, &item.path);
        let stored = join(&items, &item.path);
        let step = |facts: Facts, status: Status| Step {
            target: fence.text_of(&original),
            effect: Effect::Restore {
                stored: fence.text_of(&stored),
            },
            facts,
            held: None,
            status,
        };
        let found = match fence.existing(&stored) {
            Ok((_, entry)) => Some(survey(fence, &entry, &stored, limits.max_depth)?),
            Err(stop) if stop.reason == Reason::Missing => None,
            Err(stop) => return Err(stop),
        };
        let Some((seen, facts)) = found else {
            steps.push(step(Facts::default(), Status::Absent));
            ops.push(None);
            continue;
        };
        let parents = original.len() - 1;
        if missing_folders(fence, &original, work.len(), parents)?.is_empty()
            && fence.locate(&original)?.entry.is_some()
        {
            return Err(Stop::new(
                Reason::RestorePlaceTaken,
                format!("{} exists", fence.text_of(&original)),
            ));
        }
        steps.push(step(facts, Status::Pending));
        ops.push(Some(Op {
            rel: stored,
            seen,
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
    Ok(Job::new(work, steps, ops))
}
