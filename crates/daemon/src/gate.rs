//! Composition of the file executor with the anchored gate journal.

use std::{
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use ads_adapter_fs::{Effects, Executor, Failure, Reason, Report, Stop};
use ads_contracts::{EventEnvelopeV1, EventEnvelopeV1Stream};
use ads_core::gate::{
    AbsPath, ActionPlan, ActionPlanRecord, ActionSnapshot, Command, Decision, Estimate, Event,
    EventMeta, GateHistory, GateJournal, GatePolicy, Id, JournalError, JournalGate,
    OperationRequest, PlanError, PlanOperation, PlanRequest, State, TargetRequest, classify,
};
use ads_storage::EventLog;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::api::checked;
use crate::approvals::not_approvable;
use crate::audit::ProductLog;
use crate::events::new_id;

/// The type of the record of concrete effects and the family of its `effects/<action_id>`
/// stream.
const EFFECTS: &str = "urn:ads:event:effects.recorded:1";

/// Suffixes SQLite gives the files beside the database; losing one loses committed history.
const DATABASE_SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// Links followed while resolving one product path before it is refused.
const MAX_LINKS: usize = 32;

/// The files and folders that hold the state of the product itself: no action of a session
/// may touch them, whatever the user policy says. Built only with the log it protects
/// ([`crate::audit::open_product`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductState {
    paths: Vec<AbsPath>,
}

impl ProductState {
    /// The audit database as configured, with its side files, the vault folder that holds the
    /// binding lock and, when the daemon was started by the launcher, the folder of the
    /// instance lock it holds. Refuses what it cannot resolve.
    pub(crate) fn resolve(
        database: &Path,
        vault_dir: &Path,
        control_dir: Option<&Path>,
    ) -> Result<Self, String> {
        let mut sources = vec![database.to_path_buf()];
        for suffix in DATABASE_SIDECARS {
            let mut name = OsString::from(database.as_os_str());
            name.push(suffix);
            sources.push(PathBuf::from(name));
        }
        sources.push(vault_dir.to_path_buf());
        sources.extend(control_dir.map(Path::to_path_buf));
        let mut paths = Vec::new();
        for source in &sources {
            for path in spellings(source)? {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &[AbsPath] {
        &self.paths
    }
}

/// Every spelling under which `path` is protected: as written, as the system resolves it, and
/// each link met on the way, at its real place, since removing one breaks the path. A missing
/// entry is accepted (a file not created yet) and ends the walk; a link whose target is
/// missing or written with `..` is refused, as is a chain longer than [`MAX_LINKS`].
pub(crate) fn spellings(path: &Path) -> Result<Vec<AbsPath>, String> {
    let failed = |at: &Path, error: std::io::Error| format!("{}: {error}", at.display());
    let mut found = Vec::new();
    let mut push = |path: AbsPath| {
        if !found.contains(&path) {
            found.push(path);
        }
    };
    push(spelling(path)?);
    let mut current = path.to_path_buf();
    for _ in 0..=MAX_LINKS {
        if !current.is_absolute() {
            return Err(format!(
                "product path is not absolute: {}",
                current.display()
            ));
        }
        let mut walked = PathBuf::new();
        let mut components = current.components();
        let mut existing = current.clone();
        let mut next = None;
        while let Some(component) = components.next() {
            walked.push(component);
            match component {
                Component::Prefix(_) | Component::RootDir => continue,
                Component::Normal(_) => {}
                _ => {
                    return Err(format!(
                        "product path is not normalized: {}",
                        current.display()
                    ));
                }
            }
            let meta = match fs::symlink_metadata(&walked) {
                Ok(meta) => meta,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    existing = walked.parent().unwrap_or(&walked).to_path_buf();
                    break;
                }
                Err(error) => return Err(failed(&walked, error)),
            };
            // Junctions and the other name surrogates count as links here.
            if !meta.file_type().is_symlink() {
                continue;
            }
            let parent = walked.parent().unwrap_or(&walked);
            let real_parent = fs::canonicalize(parent).map_err(|error| failed(parent, error))?;
            if let Some(name) = walked.file_name() {
                push(spelling(&real_parent.join(name))?);
            }
            let target = fs::read_link(&walked).map_err(|error| failed(&walked, error))?;
            if target
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            {
                return Err(format!("link target uses `..`: {}", walked.display()));
            }
            let target = real_parent.join(target);
            match fs::symlink_metadata(&target) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return Err(format!("link target is missing: {}", walked.display()));
                }
                Err(error) => return Err(failed(&target, error)),
            }
            next = Some(target.join(components.as_path()));
            break;
        }
        match next {
            Some(target) => current = target,
            None => {
                let rest = current
                    .strip_prefix(&existing)
                    .map_err(|error| error.to_string())?;
                let resolved =
                    fs::canonicalize(&existing).map_err(|error| failed(&existing, error))?;
                push(spelling(&resolved.join(rest))?);
                return Ok(found);
            }
        }
    }
    Err(format!("too many links: {}", path.display()))
}

/// The text of `path` as the gate compares it. The resolved spelling of a Windows path
/// carries the `\\?\` prefix; a network share is refused.
fn spelling(path: &Path) -> Result<AbsPath, String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("product path is not valid text: {}", path.display()))?;
    let text = match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.starts_with(r"UNC\") => {
            return Err(format!("product path is on a network share: {text}"));
        }
        Some(rest) => rest,
        None => text,
    };
    AbsPath::parse(text).map_err(|error| format!("{text}: {error}"))
}

#[derive(Debug)]
pub enum PlanActionError {
    Invalid(PlanError),
    Inspect(Stop),
    Journal(JournalError),
}

/// Binds actions to one session folder and to the audit log the gate was built for: a log
/// that carries another product state is refused with [`JournalError::PolicyChanged`].
pub struct SessionGate {
    root: AbsPath,
    policy: GatePolicy,
    product: ProductState,
    executor: Executor,
}

impl SessionGate {
    pub(crate) fn new(
        root: AbsPath,
        roots: Vec<AbsPath>,
        policy: GatePolicy,
        product: &ProductLog,
        fence: &Path,
    ) -> Result<Self, String> {
        let product = product.state().clone();
        if !roots.contains(&root) {
            return Err("session working folder is no longer configured".to_owned());
        }
        let policy = policy
            .with_work_roots(roots)
            .with_product_paths(product.paths().iter().cloned());
        let executor = Executor::new(fence, policy.clone()).map_err(|error| error.to_string())?;
        Ok(Self {
            root,
            policy,
            product,
            executor,
        })
    }

    /// The session folder whose actions this gate answers for.
    pub fn work_root(&self) -> &AbsPath {
        &self.root
    }

    /// The executor with the session policy, to check what it refuses on its own.
    #[cfg(test)]
    pub(crate) fn executor_mut(&mut self) -> &mut Executor {
        &mut self.executor
    }

    /// The same gate with the executor under `limits`.
    #[cfg(test)]
    pub(crate) fn with_limits(self, limits: ads_adapter_fs::Limits) -> Self {
        Self {
            executor: self.executor.with_limits(limits),
            ..self
        }
    }

    fn bound<'a>(&self, log: &'a mut ProductLog) -> Result<&'a mut EventLog, JournalError> {
        if log.state() != &self.product {
            return Err(JournalError::PolicyChanged);
        }
        Ok(log.log())
    }

    pub fn plan(
        &self,
        log: &mut ProductLog,
        action_id: Id,
        mut request: PlanRequest,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, PlanActionError> {
        let log = self.bound(log).map_err(PlanActionError::Journal)?;
        request.work_root = self.root.to_string();
        let plan = ActionPlan::normalize(&request).map_err(PlanActionError::Invalid)?;
        let observed = self
            .executor
            .observe_plan(&plan)
            .map_err(PlanActionError::Inspect)?;
        let cause = meta.id.clone();
        let ts = meta.ts.clone();
        let snapshot = self
            .journal(log)
            .plan(action_id, &observed, meta)
            .map_err(PlanActionError::Journal)?;
        // What the approval will have to cover. A rehearsal that is refused, or a record that
        // cannot be written, leaves none: such a plan stays unapprovable where it needs one.
        if let Ok(verdict) = classify(&observed, &self.policy)
            && verdict.decision() == Decision::RequiresApproval
            && verdict.plan().hash() == snapshot.plan_hash
            && let Ok(effects) = self
                .executor
                .rehearse_effects(&snapshot.action_id, verdict.plan())
        {
            let _ = record_effects(log, &effects, cause.as_str(), ts.as_str());
        }
        Ok(snapshot)
    }

    pub fn inspect(
        &self,
        log: &mut ProductLog,
        action_id: &Id,
    ) -> Result<ActionSnapshot, JournalError> {
        let log = self.bound(log)?;
        self.journal(log).inspect(action_id)
    }

    pub fn apply(
        &self,
        log: &mut ProductLog,
        action_id: &Id,
        command: Command,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        let log = self.bound(log)?;
        self.journal(log).apply(action_id, command, meta)
    }

    fn journal<'a>(&'a self, log: &'a mut EventLog) -> JournalGate<'a> {
        JournalGate::new(&self.policy, log).for_work_root(&self.root)
    }

    /// Records the start before touching disk and uses it as the earliest outcome stamp.
    /// A failed outcome write preserves the operation report and leaves the start spent.
    pub fn execute(
        &mut self,
        log: &mut ProductLog,
        action_id: &Id,
        start: EventMeta,
        mut finish: EventMeta,
    ) -> Result<ExecutionOutcome, JournalError> {
        let log = self.bound(log)?;
        if finish.ts.is_before(&start.ts) {
            finish.ts = start.ts.clone();
        }
        // Before the license: the effects the owner approved have to be the ones on disk now.
        let approved = match self.approved_effects(log, action_id)? {
            Approved::Effects(effects) => Some(effects),
            Approved::NotNeeded => None,
            Approved::Refused(stop) => {
                let command = Command::Fail {
                    error_code: stop.reason.error_code(),
                };
                let mut gate = JournalGate::new(&self.policy, log).for_work_root(&self.root);
                return Ok(ExecutionOutcome {
                    operation: Err(Failure::before_anything(stop)),
                    recorded: gate.apply(action_id, command, start),
                });
            }
        };
        let mut gate = JournalGate::new(&self.policy, log).for_work_root(&self.root);
        let started = gate.authorize(action_id, start)?;
        let ran = match &approved {
            Some(effects) => self.executor.execute_approved(started, effects),
            None => self.executor.execute_recorded(started),
        };
        match ran {
            Ok(completed) => {
                let (recorded, report) = completed.record(&mut gate, action_id, finish);
                Ok(ExecutionOutcome {
                    operation: Ok(report),
                    recorded,
                })
            }
            Err(failure) => {
                let command = Command::Fail {
                    error_code: failure.reason.error_code(),
                };
                Ok(ExecutionOutcome {
                    operation: Err(failure),
                    recorded: gate.apply(action_id, command, finish),
                })
            }
        }
    }
}

enum Approved {
    /// Approved with this record: the execution has to find it again.
    Effects(Effects),
    /// No approval with a record of effects, and none is needed.
    NotNeeded,
    /// Refused before the license.
    Refused(Stop),
}

impl SessionGate {
    /// The record of effects an approved action has to find on disk. One is needed when the
    /// answer that approved it names one, and whenever the plan alone does not fix what the
    /// executor will do; then it has to be the one recorded, and a rehearsal now has to find
    /// it again.
    fn approved_effects(
        &self,
        log: &mut EventLog,
        action_id: &Id,
    ) -> Result<Approved, JournalError> {
        let state = self.journal(log).inspect(action_id)?.state;
        if !matches!(state, State::Approved { .. }) {
            // Not approved: the gate refuses the license, or none is asked for.
            return Ok(Approved::NotNeeded);
        }
        let history = log.load(action_id)?;
        let fields = history.plan.fields();
        let needed = !not_approvable(fields, false).is_empty();
        let refused =
            |detail: &str| Ok(Approved::Refused(Stop::new(Reason::EffectsChanged, detail)));
        let Some(committed) = approved_hash(log, &history)? else {
            if needed {
                return refused("the approval names no record of effects");
            }
            return Ok(Approved::NotNeeded);
        };
        let Some((recorded, hash)) = recorded_effects(log, &history.plan)? else {
            return refused("the record of effects the approval names is missing");
        };
        if hash != committed {
            return refused("the approval names another record of effects");
        }
        let Some(plan) = plan_of(&self.policy, &history.plan) else {
            return refused("the plan is not the one recorded under the current policy");
        };
        match self.executor.rehearse_effects(action_id, &plan) {
            Ok(found) if found == recorded => Ok(Approved::Effects(recorded)),
            Ok(_) => refused("the disk no longer holds what was approved"),
            Err(stop) => Ok(Approved::Refused(Stop::new(
                Reason::EffectsChanged,
                format!("the rehearsal is refused now: {stop}"),
            ))),
        }
    }
}

/// The plan the gate hands the executor for `record` (the quarantine rewrite, when there is
/// one), judged again under `policy`; `None` when it is not the recorded one any more.
fn plan_of(policy: &GatePolicy, record: &ActionPlanRecord) -> Option<ActionPlan> {
    let fields = record.fields();
    let operation = match fields.operation {
        PlanOperation::Delete => OperationRequest::Delete,
        PlanOperation::DeleteRecursive => OperationRequest::DeleteRecursive,
        PlanOperation::Move => OperationRequest::Move {
            destination: fields.destination.as_ref()?.as_str().to_owned(),
        },
        PlanOperation::Overwrite => OperationRequest::Overwrite,
        PlanOperation::CleanUntracked => OperationRequest::CleanUntracked,
    };
    let plan = ActionPlan::normalize(&PlanRequest {
        operation,
        work_root: fields.work_root.as_str().to_owned(),
        targets: fields
            .targets
            .iter()
            .map(|target| TargetRequest {
                path: target.text(),
                repo_root: target.repo_root,
            })
            .collect(),
        estimate: fields.estimate.map(|estimate| Estimate {
            items: estimate.items,
            bytes: estimate.bytes,
        }),
    })
    .ok()?;
    let verdict = classify(&plan, policy).ok()?;
    (verdict.plan().hash() == fields.plan_hash).then(|| verdict.plan().clone())
}

/// `sha256:` and the digest of the record as it is stored.
pub(crate) fn effects_hash(effects: &Effects) -> String {
    format!("sha256:{:x}", Sha256::digest(effects.to_bytes()))
}

fn effects_stream(action_id: &str) -> Result<EventEnvelopeV1Stream, JournalError> {
    serde_json::from_value(json!(format!("effects/{action_id}")))
        .map_err(|_| JournalError::Integrity)
}

/// Writes the record once, at `seq` 1 of `effects/<action_id>`: the log refuses a second one.
fn record_effects(
    log: &mut EventLog,
    effects: &Effects,
    cause: &str,
    ts: &str,
) -> Result<(), JournalError> {
    let stream = effects_stream(&effects.action_id)?;
    if log.head(&stream).map_err(|_| JournalError::Unavailable)? != 0 {
        return Err(JournalError::Conflict);
    }
    let value = json!({
        "id": new_id().map_err(|_| JournalError::Unavailable)?,
        "type": EFFECTS, "ts": ts, "stream": stream, "seq": 1,
        "actor": {"kind": "daemon", "id": "core"},
        "cause": cause, "data_class": "internal",
        "data": {
            "action_id": effects.action_id, "plan_hash": effects.plan_hash,
            "effects_hash": effects_hash(effects), "effects": effects,
        }
    });
    let value = checked(EFFECTS, value).map_err(|_| JournalError::Integrity)?;
    let event: EventEnvelopeV1 =
        serde_json::from_value(value).map_err(|_| JournalError::Integrity)?;
    log.append(&event).map_err(|_| JournalError::Unavailable)?;
    Ok(())
}

/// The record of effects of the plan `record`, with its hash, if the rehearsal wrote one. A
/// second record, one of another plan or one whose hash does not match is an integrity error.
pub(crate) fn recorded_effects(
    log: &mut EventLog,
    record: &ActionPlanRecord,
) -> Result<Option<(Effects, String)>, JournalError> {
    let fields = record.fields();
    let stream = effects_stream(fields.action_id.as_str())?;
    let page = log
        .read_stream(&stream, 1, 2)
        .map_err(|_| JournalError::Unavailable)?;
    let stored = match &page[..] {
        [] => return Ok(None),
        [one] => one,
        _ => return Err(JournalError::Integrity),
    };
    let event = serde_json::to_value(&stored.event).map_err(|_| JournalError::Integrity)?;
    let event = checked(EFFECTS, event).map_err(|_| JournalError::Integrity)?;
    let data = &event["data"];
    let effects: Effects =
        serde_json::from_value(data["effects"].clone()).map_err(|_| JournalError::Integrity)?;
    let hash = effects_hash(&effects);
    let plan_hash = fields.plan_hash.as_str();
    if event["type"] != EFFECTS
        || effects.version != ads_adapter_fs::EFFECTS_VERSION
        || effects.action_id != fields.action_id.as_str()
        || effects.plan_hash != plan_hash
        || data["action_id"] != fields.action_id.as_str()
        || data["plan_hash"] != plan_hash
        || data["effects_hash"] != hash.as_str()
    {
        return Err(JournalError::Integrity);
    }
    Ok(Some((effects, hash)))
}

/// The `effects_hash` of the answer that caused the grant in force; `None` when that answer
/// names none or the grant came another way.
fn approved_hash(
    log: &mut EventLog,
    history: &GateHistory,
) -> Result<Option<String>, JournalError> {
    let Some(granted) = history
        .events
        .iter()
        .rev()
        .find(|event| matches!(event, Event::ApprovalGranted(_)))
    else {
        return Ok(None);
    };
    let granted = granted.to_json();
    let Some(cause) = granted["cause"].as_str() else {
        return Ok(None);
    };
    let approval = granted["data"]["approval_id"]
        .as_str()
        .ok_or(JournalError::Integrity)?;
    let Some(answer) = crate::approvals::answer_by_id(log, approval, cause)? else {
        return Ok(None);
    };
    let data = &answer["data"];
    let fields = history.plan.fields();
    if data["action_id"] != fields.action_id.as_str()
        || data["decision"] != "approve"
        || data["plan_hash"] != fields.plan_hash.as_str()
    {
        return Err(JournalError::Integrity);
    }
    match data.get("effects_hash") {
        Some(Value::String(hash)) => Ok(Some(hash.clone())),
        None | Some(Value::Null) => Ok(None),
        Some(_) => Err(JournalError::Integrity),
    }
}

pub struct ExecutionOutcome {
    pub operation: Result<Report, Box<Failure>>,
    pub recorded: Result<ActionSnapshot, JournalError>,
}
