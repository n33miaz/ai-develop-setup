//! Composition of the file executor with the anchored gate journal.

use std::{
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    sync::MutexGuard,
};

use ads_adapter_fs::{Effects, Executor, Failure, Reason, Report, Stop};
use ads_contracts::{EventEnvelopeV1, EventEnvelopeV1Stream};
use ads_core::gate::{
    AbsPath, ActionPlan, ActionPlanRecord, ActionSnapshot, Command, Decision, Estimate, Event,
    EventMeta, GateHistory, GateJournal, GatePolicy, Id, JournalError, JournalGate,
    OperationRequest, PlanError, PlanOperation, PlanRequest, StartedExecution, State,
    TargetRequest, classify,
};
use ads_storage::{EventLog, PendingReads, StorageError, StoredEvent};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::api::checked;
use crate::audit::ProductLog;
use crate::events::new_id;

/// The type of the record of concrete effects and the family of its `effects/<action_id>`
/// stream.
const EFFECTS: &str = "urn:ads:event:effects.recorded:1";

/// The type of a rehearsal that recorded no effects, in the same stream and in their place.
const REFUSED: &str = "urn:ads:event:effects.refused:1";

/// The code of a record the core could not write (the log refused it, or the contract did).
const UNRECORDABLE: &str = "E_EFFECTS_UNRECORDABLE";

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
    resolve_spellings(path, false)
}

/// Work folders may be offline. Keep the link and its target protected even while absent.
pub(crate) fn workdir_spellings(path: &Path) -> Result<Vec<AbsPath>, String> {
    resolve_spellings(path, true)
}

fn resolve_spellings(path: &Path, allow_missing_target: bool) -> Result<Vec<AbsPath>, String> {
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
                Err(error) if allow_missing_target && error.kind() == ErrorKind::NotFound => {}
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
    /// Called once the license is spent and before the executor starts.
    #[cfg(test)]
    pub(crate) before_execute: Option<Box<dyn FnOnce() + Send>>,
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
            #[cfg(test)]
            before_execute: None,
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

    /// The same gate, calling `hook` between the license and the executor.
    #[cfg(test)]
    pub(crate) fn after_license(mut self, hook: impl FnOnce() + Send + 'static) -> Self {
        self.before_execute = Some(Box::new(hook));
        self
    }

    /// The same gate with the executor under `limits`.
    #[cfg(test)]
    pub(crate) fn with_limits(self, limits: ads_adapter_fs::Limits) -> Self {
        Self {
            executor: self.executor.with_limits(limits),
            ..self
        }
    }

    fn bound<'a>(&self, log: &'a ProductLog) -> Result<MutexGuard<'a, EventLog>, JournalError> {
        if log.state() != &self.product {
            return Err(JournalError::PolicyChanged);
        }
        log.log().map_err(|error| match error {
            ads_storage::StorageError::Busy => JournalError::Busy,
            _ => JournalError::Unavailable,
        })
    }

    pub fn plan(
        &self,
        log: &mut ProductLog,
        action_id: Id,
        mut request: PlanRequest,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, PlanActionError> {
        let mut guard = self.bound(log).map_err(PlanActionError::Journal)?;
        let log: &mut EventLog = &mut guard;
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
        // cannot be written, is recorded with its code instead: such a plan stays unapprovable
        // where it needs one, and the listing says why.
        let Ok(verdict) = classify(&observed, &self.policy) else {
            return Ok(snapshot);
        };
        if verdict.decision() != Decision::RequiresApproval {
            return Ok(snapshot);
        }
        let at = Rehearsal {
            action_id: snapshot.action_id.as_str(),
            plan_hash: snapshot.plan_hash.as_str(),
            cause: cause.as_str(),
            ts: ts.as_str(),
        };
        let written = if verdict.plan().hash() != snapshot.plan_hash {
            record_refusal(log, &at, UNRECORDABLE)
        } else {
            match self
                .executor
                .rehearse_effects(&snapshot.action_id, verdict.plan())
            {
                Ok(effects) => match record_effects(log, &effects, &at) {
                    Err(JournalError::Integrity) => record_refusal(log, &at, UNRECORDABLE),
                    other => other,
                },
                Err(stop) => record_refusal(log, &at, stop.reason.as_str()),
            }
        };
        if let Err(error) = written {
            log::warn!(
                "effects of {} not recorded: {error}",
                snapshot.action_id.as_str()
            );
        }
        Ok(snapshot)
    }

    pub fn inspect(
        &self,
        log: &mut ProductLog,
        action_id: &Id,
    ) -> Result<ActionSnapshot, JournalError> {
        let mut guard = self.bound(log)?;
        let log: &mut EventLog = &mut guard;
        self.journal(log).inspect(action_id)
    }

    pub fn apply(
        &self,
        log: &mut ProductLog,
        action_id: &Id,
        command: Command,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        let mut guard = self.bound(log)?;
        let log: &mut EventLog = &mut guard;
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
        let mut guard = self.bound(log)?;
        let log: &mut EventLog = &mut guard;
        if finish.ts.is_before(&start.ts) {
            finish.ts = start.ts.clone();
        }
        // Before the license: the effects the owner approved have to be the ones on disk now.
        let approved = match self.approved_effects(log, action_id)? {
            Approved::Verified(approval) => ExecutionEvidence::Approved(approval),
            Approved::NotNeeded => ExecutionEvidence::WithoutApproval,
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
        let license = FileExecutionLicense {
            started: gate.authorize(action_id, start)?,
            evidence: approved,
        };
        #[cfg(test)]
        if let Some(before_execute) = self.before_execute.take() {
            before_execute();
        }
        let ran = license.run(&mut self.executor);
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

struct VerifiedApproval(Effects);

enum ExecutionEvidence {
    WithoutApproval,
    Approved(VerifiedApproval),
}

/// Licença de execução da composição local.
///
/// ```compile_fail,E0451
/// fn fabricate(started: ads_core::gate::StartedExecution) {
///     let _ = ads_daemon::gate::FileExecutionLicense { started, evidence: todo!() };
/// }
/// ```
pub struct FileExecutionLicense {
    started: StartedExecution,
    evidence: ExecutionEvidence,
}

impl FileExecutionLicense {
    fn run(self, executor: &mut Executor) -> Result<ads_adapter_fs::Completed, Box<Failure>> {
        match self.evidence {
            ExecutionEvidence::WithoutApproval if self.started.permit().approval_id().is_none() => {
                executor.execute_recorded(self.started)
            }
            ExecutionEvidence::WithoutApproval => Err(Failure::before_anything(Stop::new(
                Reason::EffectsChanged,
                "the started approval has no verified effects",
            ))),
            ExecutionEvidence::Approved(VerifiedApproval(effects)) => {
                executor.execute_approved(self.started, &effects)
            }
        }
    }
}

enum Approved {
    /// Approved with this record: the execution has to find it again.
    Verified(VerifiedApproval),
    /// No approval to verify on this path.
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
        let snapshot = self.journal(log).inspect(action_id)?;
        if snapshot.policy_warning.is_some() || !matches!(snapshot.state, State::Approved { .. }) {
            // Not approved, or approved under another policy: the gate refuses the license and
            // records why, or none is asked for.
            return Ok(Approved::NotNeeded);
        }
        let history = log.load(action_id)?;
        let refused =
            |detail: &str| Ok(Approved::Refused(Stop::new(Reason::EffectsChanged, detail)));
        let Some(committed) = approved_hash(log, &history)? else {
            return refused("the approval has no local answer naming a record of effects");
        };
        let Recorded::Effects(recorded, hash) = recorded_effects(log, &history.plan)? else {
            return refused("the record of effects the approval names is missing");
        };
        if hash != committed {
            return refused("the approval names another record of effects");
        }
        let Some(plan) = plan_of(&self.policy, &history.plan) else {
            return refused("the plan is not the one recorded under the current policy");
        };
        match self.executor.rehearse_effects(action_id, &plan) {
            Ok(found) if found == recorded => Ok(Approved::Verified(VerifiedApproval(recorded))),
            Ok(_) => refused("the disk no longer holds what was approved"),
            Err(stop) => Ok(Approved::Refused(stop)),
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

/// Where a rehearsal at planning time writes what it found.
struct Rehearsal<'a> {
    action_id: &'a str,
    plan_hash: &'a str,
    cause: &'a str,
    ts: &'a str,
}

/// Appends the only event of `effects/<action_id>`, at `seq` 1: the log refuses a second one.
fn append_once(
    log: &mut EventLog,
    kind: &str,
    at: &Rehearsal<'_>,
    data: Value,
) -> Result<(), JournalError> {
    let stream = effects_stream(at.action_id)?;
    if log.head(&stream).map_err(|_| JournalError::Unavailable)? != 0 {
        return Err(JournalError::Conflict);
    }
    let value = json!({
        "id": new_id().map_err(|_| JournalError::Unavailable)?,
        "type": kind, "ts": at.ts, "stream": stream, "seq": 1,
        "actor": {"kind": "daemon", "id": "core"},
        "cause": at.cause, "data_class": "internal", "data": data,
    });
    let value = checked(kind, value).map_err(|_| JournalError::Integrity)?;
    let event: EventEnvelopeV1 =
        serde_json::from_value(value).map_err(|_| JournalError::Integrity)?;
    match log.append(&event) {
        Ok(_) => Ok(()),
        // Over `max_event_bytes`, or refused by a check of the log: the record cannot be kept.
        Err(ads_storage::StorageError::TooLarge | ads_storage::StorageError::InvalidEvent) => {
            Err(JournalError::Integrity)
        }
        Err(_) => Err(JournalError::Unavailable),
    }
}

fn record_effects(
    log: &mut EventLog,
    effects: &Effects,
    at: &Rehearsal<'_>,
) -> Result<(), JournalError> {
    let data = json!({
        "action_id": effects.action_id, "plan_hash": effects.plan_hash,
        "effects_hash": effects_hash(effects), "effects": effects,
    });
    append_once(log, EFFECTS, at, data)
}

fn record_refusal(log: &mut EventLog, at: &Rehearsal<'_>, code: &str) -> Result<(), JournalError> {
    let data = json!({"action_id": at.action_id, "plan_hash": at.plan_hash, "error_code": code});
    append_once(log, REFUSED, at, data)
}

/// What the rehearsal at planning time left for an action.
#[derive(Debug)]
pub(crate) enum Recorded {
    /// Nothing: the plan asked for no approval, or the log lost the write.
    Nothing,
    /// The record of effects and its hash.
    Effects(Effects, String),
    /// The rehearsal was refused, with this code.
    Refused(String),
}

/// Where a record of effects is read: the log, or the transaction of a listing.
pub(crate) trait EffectsSource {
    fn read(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError>;
    /// The stored size of each row, without copying or decrypting it.
    fn sizes(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<usize>, StorageError>;
}

impl EffectsSource for EventLog {
    fn read(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        self.read_stream(stream, from_seq, limit)
    }
    fn sizes(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<usize>, StorageError> {
        self.stream_sizes(stream, from_seq, limit)
    }
}

impl EffectsSource for PendingReads<'_> {
    fn read(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        self.read_stream(stream, from_seq, limit)
    }
    fn sizes(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<usize>, StorageError> {
        self.stream_sizes(stream, from_seq, limit)
    }
}

#[cfg(test)]
thread_local! {
    /// How many times this thread asked the log for a record of effects.
    pub(crate) static RECORDS_READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The stored size of the record of effects of the plan `record`, read before the record
/// itself; `None` when there is none. A second event is an integrity error, as in
/// [`recorded_effects`].
pub(crate) fn effects_size(
    log: &(impl EffectsSource + ?Sized),
    record: &ActionPlanRecord,
) -> Result<Option<usize>, JournalError> {
    let stream = effects_stream(record.fields().action_id.as_str())?;
    match log
        .sizes(&stream, 1, 2)
        .map_err(|_| JournalError::Unavailable)?[..]
    {
        [] => Ok(None),
        [size] => Ok(Some(size)),
        _ => Err(JournalError::Integrity),
    }
}

/// What the rehearsal of the plan `record` left. A second event, one of another plan, a
/// record this core does not read (another version, an unknown field) or one whose hash does
/// not match is an integrity error.
pub(crate) fn recorded_effects(
    log: &(impl EffectsSource + ?Sized),
    record: &ActionPlanRecord,
) -> Result<Recorded, JournalError> {
    let fields = record.fields();
    let stream = effects_stream(fields.action_id.as_str())?;
    #[cfg(test)]
    RECORDS_READ.with(|read| read.set(read.get() + 1));
    let page = log
        .read(&stream, 1, 2)
        .map_err(|_| JournalError::Unavailable)?;
    let stored = match &page[..] {
        [] => return Ok(Recorded::Nothing),
        [one] => one,
        _ => return Err(JournalError::Integrity),
    };
    let event = serde_json::to_value(&stored.event).map_err(|_| JournalError::Integrity)?;
    let kind = match event["type"].as_str() {
        Some(EFFECTS) => EFFECTS,
        Some(REFUSED) => REFUSED,
        _ => return Err(JournalError::Integrity),
    };
    let event = checked(kind, event).map_err(|_| JournalError::Integrity)?;
    let data = &event["data"];
    let plan_hash = fields.plan_hash.as_str();
    if data["action_id"] != fields.action_id.as_str() || data["plan_hash"] != plan_hash {
        return Err(JournalError::Integrity);
    }
    if kind == REFUSED {
        let code = data["error_code"].as_str().ok_or(JournalError::Integrity)?;
        return Ok(Recorded::Refused(code.to_owned()));
    }
    let effects: Effects =
        serde_json::from_value(data["effects"].clone()).map_err(|_| JournalError::Integrity)?;
    let hash = effects_hash(&effects);
    if effects.version != ads_adapter_fs::EFFECTS_VERSION
        || effects.action_id != fields.action_id.as_str()
        || effects.plan_hash != plan_hash
        || data["effects_hash"] != hash.as_str()
    {
        return Err(JournalError::Integrity);
    }
    Ok(Recorded::Effects(effects, hash))
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
    let answer =
        checked("urn:ads:event:answer.recorded:2", answer).map_err(|_| JournalError::Integrity)?;
    let data = &answer["data"];
    let fields = history.plan.fields();
    if data["via"] != "local_ui"
        || granted["data"]["via"] != "local_ui"
        || answer["actor"] != granted["actor"]
        || data["approval_id"] != approval
        || data["action_id"] != fields.action_id.as_str()
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
