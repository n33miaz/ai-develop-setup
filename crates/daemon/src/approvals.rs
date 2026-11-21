//! Owner answers to the action gate for the local API: the approvals the session gates wait
//! for, and each answer recorded in the anchored audit log before the gate acts on it.

use std::num::NonZeroU64;
use std::sync::{Mutex, MutexGuard};

use ads_contracts::{
    CommandApprovalAnswerV1Params, CommandApprovalAnswerV1ParamsDecision as Decision, CommonIdV1,
    EventEnvelopeV1, EventEnvelopeV1Stream,
};
use ads_core::gate::{
    AbsPath, ActionPlanFields, Actor, ActorKind, Command, DataClass, Event, EventMeta, GateHistory,
    GateJournal, Id, JournalError, PlanHash, PlanOperation, State, Timestamp, Ulid, Via,
};
use serde_json::{Value, json};

use crate::api::{ApiError, checked};
use crate::audit::ProductLog;
use crate::auth::LocalSession;
use crate::events::{new_id, now_ts};
use crate::gate::{Recorded, SessionGate, recorded_effects};
use ads_adapter_fs::Effects;

/// The type of the answer record and the family of its `answer/<approval_id>` stream.
const ANSWER: &str = "urn:ads:event:answer.recorded:2";

/// The most approvals one `approval.list` answers: the `maxItems` of its response contract.
const MAX_LISTED: u32 = 1000;

/// What the local API asks of the gate. Each call takes the proof that the connection passed
/// the token check; the owner and the device come from the implementation, never the request.
pub trait ApprovalControl: Send + Sync + 'static {
    /// The actions waiting for the owner, oldest first, as the `result` of
    /// `command.approval.list.response:2`.
    fn pending(&self, session: &LocalSession) -> Result<Pending, ApiError>;
    /// Records the answer, then lets the gate act on it. `cause` is the command id. Returns
    /// whether the outcome is the one of an earlier answer with the same idempotency key.
    fn answer(
        &self,
        session: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<bool, ApiError>;
}

/// One page of `approval.list`: the actions that fit, oldest first, and whether the ceiling of
/// bytes left others out.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub approvals: Vec<Value>,
    pub more: bool,
}

/// No gate bound to the audit log: every approval command is refused.
pub struct DisabledApprovals;
impl ApprovalControl for DisabledApprovals {
    fn pending(&self, _: &LocalSession) -> Result<Pending, ApiError> {
        Err(ApiError::ApprovalNotConfigured)
    }
    fn answer(
        &self,
        _: &LocalSession,
        _: &CommandApprovalAnswerV1Params,
        _: &str,
    ) -> Result<bool, ApiError> {
        Err(ApiError::ApprovalNotConfigured)
    }
}

/// The owner who answers and the device id the local interface acts as, from the core's own
/// configuration.
#[derive(Debug, Clone)]
pub struct ApprovalOwner {
    pub owner_id: CommonIdV1,
    pub device_id: CommonIdV1,
}

/// Why the API does not let the owner approve a plan: its concrete effect is read from the
/// disk at execution, and neither the record the owner sees nor a record of the effects a
/// rehearsal found (`effects`) fixes it. Denying stays allowed.
pub fn not_approvable(plan: &ActionPlanFields, effects: bool) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    // Without an estimate the executor has no ceiling for what it finds in the targets.
    if plan.estimate.is_none() && !effects {
        reasons.push("estimate_unknown");
    }
    // The names a wildcard matches are chosen when the executor reads the folder.
    if plan.targets.iter().any(|target| target.pattern.is_some()) {
        reasons.push("wildcard_target");
    }
    // A tree removed for good is enumerated at execution and cannot be brought back; the
    // executor rehearses a recursive delete, never a clean of untracked files.
    let unfixed = match plan.operation {
        PlanOperation::DeleteRecursive => !effects,
        PlanOperation::CleanUntracked => true,
        _ => false,
    };
    if unfixed && !plan.quarantined {
        reasons.push("definitive_recursive_delete");
    }
    // A move back to the work root is a restore: the pairs come from the batch manifest.
    let restore = plan.operation == PlanOperation::Move
        && !plan.quarantined
        && plan
            .destination
            .as_ref()
            .is_some_and(|destination| destination.as_str() == plan.work_root.as_str());
    if restore && !effects {
        reasons.push("restore_from_manifest");
    }
    reasons
}

/// The answer recorded under `id` in `answer/<approval_id>`, as JSON.
pub(crate) fn answer_by_id(
    log: &mut ads_storage::EventLog,
    approval_id: &str,
    id: &str,
) -> Result<Option<Value>, JournalError> {
    let approval_id = Id::new(approval_id).map_err(|_| JournalError::Integrity)?;
    let stream = answer_stream(&approval_id).map_err(|_| JournalError::Integrity)?;
    find_answer(log, &stream, |event| event["id"] == id).map_err(|_| JournalError::Unavailable)
}

fn find_answer(
    log: &mut ads_storage::EventLog,
    stream: &EventEnvelopeV1Stream,
    mut wanted: impl FnMut(&Value) -> bool,
) -> Result<Option<Value>, ApiError> {
    let mut next = 1;
    loop {
        let page = log
            .read_stream(stream, next, u32::MAX)
            .map_err(|_| ApiError::Storage)?;
        let Some(last) = page.last() else {
            return Ok(None);
        };
        next = last.event.seq.get() + 1;
        for stored in page {
            let event = serde_json::to_value(stored.event).map_err(|_| ApiError::Storage)?;
            if wanted(&event) {
                return Ok(Some(event));
            }
        }
    }
}

/// The audit log and the session gates bound to it. Planning, asking for approval and executing
/// go through [`Self::with_gates`], under the same lock as the answers.
pub struct ApprovalDesk {
    inner: Mutex<Desk>,
}
struct Desk {
    log: ProductLog,
    gates: Vec<SessionGate>,
    owner: ApprovalOwner,
    /// `approval_list_max_bytes` of the API configuration.
    max_list_bytes: usize,
}

impl ApprovalDesk {
    /// `max_list_bytes` is `approval_list_max_bytes` of the API configuration: the ceiling of
    /// one `approval.list` response, and of a record of effects shown in it (half of it).
    pub fn new(
        log: ProductLog,
        gates: Vec<SessionGate>,
        owner: ApprovalOwner,
        max_list_bytes: usize,
    ) -> Self {
        Self {
            inner: Mutex::new(Desk {
                log,
                gates,
                owner,
                max_list_bytes,
            }),
        }
    }

    pub fn with_gates<R>(&self, act: impl FnOnce(&mut ProductLog, &mut [SessionGate]) -> R) -> R {
        let mut desk = self.lock();
        let Desk { log, gates, .. } = &mut *desk;
        act(log, gates)
    }

    fn lock(&self) -> MutexGuard<'_, Desk> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A record of effects is shown, and so can be approved, only when it takes at most half of
/// the page: the rest of the item and the other actions keep room.
fn listable(effects: &Effects, max_list_bytes: usize) -> bool {
    effects.to_bytes().len() <= max_list_bytes / 2
}

/// What the owner sees of the effects of one action.
enum Shown {
    /// No record, and none needed or none written.
    Nothing,
    Effects(Effects, String),
    Refused(String),
    /// A record this core does not read; logged.
    Unreadable,
    /// A record larger than [`listable`] allows.
    NotListed,
}

impl Shown {
    fn of(recorded: Result<Recorded, JournalError>, action: &Id, max_list_bytes: usize) -> Self {
        match recorded {
            Ok(Recorded::Nothing) => Self::Nothing,
            Ok(Recorded::Effects(effects, _)) if !listable(&effects, max_list_bytes) => {
                Self::NotListed
            }
            Ok(Recorded::Effects(effects, hash)) => Self::Effects(effects, hash),
            Ok(Recorded::Refused(code)) => Self::Refused(code),
            Err(error) => {
                log::warn!(
                    "record of effects of {} not readable: {error}",
                    action.as_str()
                );
                Self::Unreadable
            }
        }
    }

    fn effects(&self) -> Option<(&Effects, &str)> {
        match self {
            Self::Effects(effects, hash) => Some((effects, hash.as_str())),
            _ => None,
        }
    }

    /// Why the owner cannot approve: the reasons of the plan and, when it needs effects and
    /// has none to show, why it has none.
    fn reasons(&self, plan: &ActionPlanFields) -> Vec<&'static str> {
        let mut reasons = not_approvable(plan, self.effects().is_some());
        if not_approvable(plan, false).is_empty() {
            return reasons;
        }
        match self {
            Self::Refused(_) => reasons.push("effects_refused"),
            Self::Unreadable => reasons.push("effects_unreadable"),
            Self::NotListed => reasons.push("effects_not_listed"),
            Self::Nothing | Self::Effects(..) => {}
        }
        reasons
    }
}

fn root_of(history: &GateHistory) -> Option<AbsPath> {
    AbsPath::parse(history.plan.fields().work_root.as_str()).ok()
}

fn answer_stream(approval_id: &Id) -> Result<EventEnvelopeV1Stream, ApiError> {
    serde_json::from_value(json!(format!("answer/{}", approval_id.as_str())))
        .map_err(|_| ApiError::Invalid)
}

impl Desk {
    fn gate_for(&self, history: &GateHistory) -> Option<usize> {
        let root = root_of(history)?;
        self.gates.iter().position(|gate| gate.work_root() == &root)
    }

    /// Writes the answer as the owner's device, at the next `seq` of its stream.
    fn record(
        &mut self,
        answer: &CommandApprovalAnswerV1Params,
        effects_hash: Option<&str>,
        ts: &str,
        cause: &str,
    ) -> Result<String, ApiError> {
        let stream = answer_stream(&answer.approval_id)?;
        let log = self.log.log();
        let seq = log.head(&stream).map_err(|_| ApiError::Storage)? + 1;
        let id = new_id()?;
        let value = json!({
            "id": id, "type": ANSWER, "ts": ts, "stream": stream, "seq": seq,
            "actor": {"kind": "device", "id": self.owner.device_id},
            "cause": cause, "data_class": "internal",
            "data": {
                "approval_id": answer.approval_id, "action_id": answer.action_id,
                "plan_hash": answer.plan_hash, "effects_hash": effects_hash,
                "decision": answer.decision.as_str(),
                "owner_id": self.owner.owner_id, "via": "local_ui",
                "idempotency_key": answer.meta.idempotency_key
            }
        });
        let value = checked(ANSWER, value).map_err(|_| ApiError::Storage)?;
        let event: EventEnvelopeV1 =
            serde_json::from_value(value).map_err(|_| ApiError::Storage)?;
        log.append(&event).map_err(|_| ApiError::Storage)?;
        Ok(id)
    }

    /// The answer already recorded for this approval with the same idempotency key.
    fn recorded_with_key(
        &mut self,
        approval_id: &Id,
        key: &str,
    ) -> Result<Option<Value>, ApiError> {
        let stream = answer_stream(approval_id)?;
        find_answer(self.log.log(), &stream, |event| {
            event["data"]["idempotency_key"] == key
        })
    }
}

/// What the gate decided because of the answer `answer_id`, if it decided anything.
fn decided_by(history: &GateHistory, answer_id: &Value) -> Option<Result<(), ApiError>> {
    history.events.iter().find_map(|event| {
        if &event.to_json()["cause"] != answer_id {
            return None;
        }
        Some(match event {
            Event::ApprovalGranted(_) => Ok(()),
            Event::ApprovalDenied(denied) => {
                match denied.data().reason.as_ref().map(|r| r.as_str()) {
                    Some("E_POLICY_CHANGED" | "E_CLOCK_POLICY_CHANGED") => {
                        Err(ApiError::ApprovalPolicyChanged)
                    }
                    _ => Ok(()),
                }
            }
            Event::ApprovalExpired(_) => Err(ApiError::ApprovalStale),
            _ => return None,
        })
    })
}

impl ApprovalControl for ApprovalDesk {
    fn pending(&self, _: &LocalSession) -> Result<Pending, ApiError> {
        let (waiting, max_list_bytes) = {
            let mut desk = self.lock();
            let roots: Vec<AbsPath> = desk
                .gates
                .iter()
                .map(|gate| gate.work_root().clone())
                .collect();
            // Only what a gate here can answer; the rest belongs to another session folder and
            // never takes a place in the page.
            let mut served =
                |history: &GateHistory| root_of(history).is_some_and(|root| roots.contains(&root));
            let histories = desk
                .log
                .log()
                .pending_approvals(MAX_LISTED, &mut served)
                .map_err(|_| ApiError::Storage)?;
            let mut waiting = Vec::new();
            for history in histories {
                let recorded = recorded_effects(desk.log.log(), &history.plan);
                waiting.push((history, recorded));
            }
            (waiting, desk.max_list_bytes)
        };
        // Built out of the lock: answers, plans and executions do not wait for it.
        let mut page = Pending {
            approvals: Vec::new(),
            more: false,
        };
        // The envelope of the response and `"more":false`.
        let mut used = 128;
        for (history, recorded) in waiting {
            let Some(Event::ApprovalRequested(requested)) = history.events.last() else {
                continue;
            };
            let data = requested.data();
            let shown = Shown::of(recorded, &data.action_id, max_list_bytes);
            let effects = shown.effects();
            let item = json!({
                "action_id": data.action_id.as_str(),
                "approval_id": data.approval_id.as_str(),
                "plan_hash": data.plan_hash.as_str(),
                "risk": data.risk,
                "summary": data.summary.as_str(),
                "requested_at": requested.ts().as_str(),
                "expires_at": data.expires_at.as_str(),
                "not_approvable": shown.reasons(history.plan.fields()),
                "plan": history.plan.to_json(),
                "effects_hash": effects.map(|(_, hash)| hash),
                "effects": effects.map(|(effects, _)| effects),
                "effects_error": match &shown {
                    Shown::Refused(code) => Some(code.as_str()),
                    _ => None,
                },
            });
            let size = serde_json::to_vec(&item)
                .map_err(|_| ApiError::Storage)?
                .len()
                + 1;
            // The oldest action is always listed, so that the page never stays empty.
            if !page.approvals.is_empty() && used + size > max_list_bytes {
                page.more = true;
                break;
            }
            used += size;
            page.approvals.push(item);
        }
        Ok(page)
    }

    fn answer(
        &self,
        _: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<bool, ApiError> {
        let mut guard = self.lock();
        let desk = &mut *guard;
        let action_id: Id = answer.action_id.clone();
        let approval_id: Id = answer.approval_id.clone();
        let plan_hash = PlanHash::new(answer.plan_hash.as_str()).map_err(|_| ApiError::Invalid)?;
        let history = match desk.log.log().load(&action_id) {
            Ok(history) => history,
            Err(JournalError::Missing) => return Err(ApiError::ApprovalNotFound),
            Err(_) => return Err(ApiError::Storage),
        };
        let index = desk.gate_for(&history).ok_or(ApiError::ApprovalNotFound)?;
        // The approval id is checked against the request in the history, whatever the state.
        let asked = history.events.iter().any(|event| {
            matches!(event, Event::ApprovalRequested(requested)
                if requested.data().approval_id == approval_id)
        });
        if !asked {
            return Err(ApiError::ApprovalNotFound);
        }

        // The same key never records a second answer: its outcome is read back from the log,
        // and a decision the gate never wrote is taken again under the recorded answer.
        let earlier = desk.recorded_with_key(&approval_id, answer.meta.idempotency_key.as_str())?;
        if let Some(event) = &earlier {
            let data = &event["data"];
            if data["decision"] != answer.decision.as_str()
                || data["plan_hash"] != answer.plan_hash.as_str()
            {
                return Err(ApiError::Conflict);
            }
            if let Some(outcome) = decided_by(&history, &event["id"]) {
                return outcome.map(|()| true);
            }
        }

        let snapshot = desk.gates[index]
            .inspect(&mut desk.log, &action_id)
            .map_err(gate_error)?;
        // The record of effects the listing showed: an approval commits to its hash. Denying
        // never depends on it.
        let shown = match answer.decision {
            Decision::Approve => Shown::of(
                recorded_effects(desk.log.log(), &history.plan),
                &action_id,
                desk.max_list_bytes,
            ),
            Decision::Deny => Shown::Nothing,
        };
        let effects_hash = shown.effects().map(|(_, hash)| hash.to_owned());
        let effects_hash = effects_hash.as_deref();
        let ts = now_ts()?;
        let now = Timestamp::new(ts.as_str()).map_err(|_| ApiError::Storage)?;
        let (lapsed, current_hash) = match &snapshot.state {
            State::AwaitingApproval { expires_at, .. } => {
                (!now.is_before(expires_at), snapshot.plan_hash.clone())
            }
            _ => return Err(ApiError::ApprovalAlreadyAnswered),
        };

        // The answer is durable before the gate acts: a refused write leaves the gate untouched.
        let recorded = match &earlier {
            Some(event) => event["id"].as_str().ok_or(ApiError::Storage)?.to_owned(),
            None => desk.record(answer, effects_hash, &ts, cause)?,
        };
        let approval_stream = format!("approval/{}", approval_id.as_str());
        let seq = history
            .events
            .iter()
            .filter(|event| event.stream() == approval_stream)
            .count() as u64
            + 1;
        let meta = EventMeta {
            id: Ulid::new(new_id()?).map_err(|_| ApiError::Storage)?,
            ts: now,
            seq: NonZeroU64::new(seq).ok_or(ApiError::Storage)?,
            actor: Actor {
                kind: ActorKind::Device,
                id: desk.owner.device_id.clone(),
            },
            cause: Some(Ulid::new(recorded).map_err(|_| ApiError::Storage)?),
            data_class: DataClass::Internal,
        };
        let (command, refused) = if lapsed {
            // Too late for any answer: the gate records the expiry, caused by this answer.
            let expire = Command::Expire {
                action_id: action_id.clone(),
                approval_id,
                plan_hash: current_hash,
            };
            (expire, Some(ApiError::ApprovalStale))
        } else if current_hash != plan_hash {
            // The owner answered another plan than the one waiting: recorded, never applied.
            return Err(ApiError::ApprovalStale);
        } else if answer.decision == Decision::Approve
            && !shown.reasons(history.plan.fields()).is_empty()
        {
            // Its concrete effect is not fixed by the record: recorded, the action keeps waiting.
            return Err(ApiError::ApprovalNotApprovable);
        } else {
            let command = match answer.decision {
                Decision::Approve => Command::Grant {
                    action_id: action_id.clone(),
                    approval_id,
                    plan_hash,
                    via: Via::LocalUi,
                },
                Decision::Deny => Command::Deny {
                    action_id: action_id.clone(),
                    approval_id,
                    plan_hash,
                    reason: None,
                },
            };
            (command, None)
        };
        match desk.gates[index].apply(&mut desk.log, &action_id, command, meta) {
            Ok(_) => refused.map_or(Ok(false), Err),
            Err(JournalError::Transition) => {
                // Refused by the machine: tell a decision taken meanwhile from a stale answer.
                let state = desk.gates[index]
                    .inspect(&mut desk.log, &action_id)
                    .map_err(gate_error)?
                    .state;
                Err(match state {
                    State::AwaitingApproval { .. } => ApiError::ApprovalStale,
                    _ => ApiError::ApprovalAlreadyAnswered,
                })
            }
            Err(error) => Err(gate_error(error)),
        }
    }
}

fn gate_error(error: JournalError) -> ApiError {
    match error {
        JournalError::Missing => ApiError::ApprovalNotFound,
        JournalError::Conflict => ApiError::ApprovalAlreadyAnswered,
        JournalError::PolicyChanged
        | JournalError::PlanNormalizationFailed
        | JournalError::ClassificationBudgetExceeded(_)
        | JournalError::HistoryOutOfOrder => ApiError::ApprovalPolicyChanged,
        JournalError::OutOfOrder => ApiError::ApprovalOutOfOrder,
        JournalError::Integrity | JournalError::Transition | JournalError::Unavailable => {
            ApiError::Storage
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::TestVault;
    use crate::auth::Token;
    use ads_contracts::EventEnvelopeV1Stream;
    use ads_core::gate::{
        Estimate, GatePolicy, HostFacts, OperationRequest, PlanRequest, ShortText, TargetRequest,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    /// Session folders under one fence, their product log, its vault and the files made here.
    struct Bench {
        root: PathBuf,
        work: String,
        storage: String,
        vault: TestVault,
        log: Option<ProductLog>,
        made: Vec<PathBuf>,
    }

    impl Bench {
        fn new() -> Self {
            Self::with_storage("")
        }

        /// A bench whose log reads `storage` over the storage defaults.
        fn with_storage(storage: &str) -> Self {
            let mut unique = [0_u8; 16];
            getrandom::fill(&mut unique).unwrap();
            let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
            let root = base.join(format!("approvals-{:032x}", u128::from_be_bytes(unique)));
            fs::create_dir_all(root.join("fence").join("work")).unwrap();
            let resolved = fs::canonicalize(&root).unwrap();
            let resolved = resolved.to_str().unwrap();
            let root = PathBuf::from(resolved.strip_prefix(r"\\?\").unwrap_or(resolved));
            let work = root.join("fence").join("work").to_str().unwrap().to_owned();
            let vault = TestVault::default();
            let log = crate::audit::test_product_log(&root, storage, &vault);
            let mut bench = Self {
                work,
                root,
                storage: storage.to_owned(),
                vault,
                log: Some(log),
                made: Vec::new(),
            };
            let build = bench.at(&bench.work.clone(), "build.log");
            bench.file(&build);
            bench
        }

        fn at(&self, folder: &str, name: &str) -> String {
            format!("{folder}{}{name}", std::path::MAIN_SEPARATOR)
        }

        /// Writes a file this bench removes when it is dropped.
        fn file(&mut self, path: &str) {
            fs::write(path, "content").unwrap();
            self.made.push(PathBuf::from(path));
        }

        /// A second session folder beside the first one, with its own `build.log`.
        fn other_work(&mut self) -> String {
            let other = self.root.join("fence").join("other");
            fs::create_dir_all(&other).unwrap();
            let other = other.to_str().unwrap().to_owned();
            let build = self.at(&other, "build.log");
            self.file(&build);
            other
        }

        fn fence(&self) -> PathBuf {
            self.root.join("fence")
        }

        fn gate(&self, overrides: &str) -> SessionGate {
            self.gate_in(&self.work.clone(), overrides)
        }

        fn gate_in(&self, work: &str, overrides: &str) -> SessionGate {
            let home = self.fence().join("fake-home");
            let policy = GatePolicy::from_toml_str(
                overrides,
                &HostFacts {
                    user_home: home.to_str().unwrap(),
                },
            )
            .unwrap();
            let root = AbsPath::parse(work).unwrap();
            SessionGate::new(
                root.clone(),
                vec![root],
                policy,
                self.log.as_ref().unwrap(),
                &self.fence(),
            )
            .unwrap()
            .with_limits(limits(""))
        }

        /// The log opened again on the same database and vault.
        fn reopen(&self) -> ProductLog {
            crate::audit::test_product_log(&self.root, &self.storage, &self.vault)
        }

        /// Plans a delete of 10 items, over the approval limit of `OVER_LIMIT`, and asks for
        /// approval. Returns the action, its approval and its plan hash.
        fn waiting(&mut self, gate: &SessionGate, name: &str) -> (Id, Id, String) {
            self.waiting_since(gate, name, 0)
        }

        /// Like [`Self::waiting`], planned and asked `hours` ago with a validity of one hour.
        fn waiting_since(
            &mut self,
            gate: &SessionGate,
            name: &str,
            hours: i64,
        ) -> (Id, Id, String) {
            let work = gate.work_root().to_string();
            let request = PlanRequest {
                operation: OperationRequest::Delete,
                targets: vec![TargetRequest {
                    path: self.at(&work, "build.log"),
                    repo_root: false,
                }],
                work_root: work,
                estimate: Some(Estimate {
                    items: 10,
                    bytes: 3,
                }),
            };
            self.ask(gate, name, request, hours)
        }

        /// Plans `request`, `hours` ago, and asks for approval with a validity of one hour.
        fn ask(
            &mut self,
            gate: &SessionGate,
            name: &str,
            request: PlanRequest,
            hours: i64,
        ) -> (Id, Id, String) {
            let at = |offset: i64| {
                (OffsetDateTime::now_utc() + Duration::hours(offset))
                    .replace_nanosecond(0)
                    .unwrap()
                    .format(&Rfc3339)
                    .unwrap()
            };
            let asked_at = at(-hours);
            let log = self.log.as_mut().unwrap();
            let action_id = Id::new(name).unwrap();
            let approval_id = Id::new(format!("appr-{name}")).unwrap();
            let planned = gate
                .plan(log, action_id.clone(), request, daemon_meta(1, &asked_at))
                .unwrap();
            assert_eq!(planned.state, State::Planned);
            let expires = at(1 - hours);
            let asked = gate
                .apply(
                    log,
                    &action_id,
                    Command::RequestApproval {
                        approval_id: approval_id.clone(),
                        summary: ShortText::new("Remove what the plan names").unwrap(),
                        expires_at: Timestamp::new(expires).unwrap(),
                    },
                    daemon_meta(1, &asked_at),
                )
                .unwrap();
            (action_id, approval_id, asked.plan_hash.as_str().to_owned())
        }

        fn desk(&mut self, gates: Vec<SessionGate>) -> ApprovalDesk {
            desk_over(self.log.take().unwrap(), gates)
        }
    }

    fn desk_over(log: ProductLog, gates: Vec<SessionGate>) -> ApprovalDesk {
        desk_with(log, gates, list_bytes())
    }

    /// The default ceiling of `approval.list`.
    fn list_bytes() -> usize {
        crate::api::ApiConfig::from_toml_str("")
            .unwrap()
            .approval_list_max_bytes
    }

    fn desk_with(log: ProductLog, gates: Vec<SessionGate>, max_list_bytes: usize) -> ApprovalDesk {
        ApprovalDesk::new(
            log,
            gates,
            ApprovalOwner {
                owner_id: Id::new("owner-01").unwrap(),
                device_id: Id::new("device-01").unwrap(),
            },
            max_list_bytes,
        )
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            drop(self.log.take());
            // Only the files this bench made, one by one; the folders stay in `.test-tmp`.
            for path in &self.made {
                let _ = fs::remove_file(path);
            }
            for suffix in ["", "-wal", "-shm", "-journal"] {
                let name = format!("audit.db{suffix}");
                let _ = fs::remove_file(self.root.join("audit").join(name));
            }
        }
    }

    fn daemon_meta(seq: u64, ts: &str) -> EventMeta {
        EventMeta {
            id: Ulid::new(new_id().unwrap()).unwrap(),
            ts: Timestamp::new(ts).unwrap(),
            seq: NonZeroU64::new(seq).unwrap(),
            actor: Actor {
                kind: ActorKind::Daemon,
                id: Id::new("core").unwrap(),
            },
            cause: None,
            data_class: DataClass::Internal,
        }
    }

    fn session() -> LocalSession {
        let token = Token::generate().unwrap();
        let url = token.launch_url("http://127.0.0.1:5173", 1);
        token
            .open_session(url.rsplit("token=").next().unwrap())
            .unwrap()
    }

    fn params(
        action: &Id,
        approval: &Id,
        hash: &str,
        decision: &str,
    ) -> CommandApprovalAnswerV1Params {
        serde_json::from_value(json!({
            "action_id": action, "approval_id": approval, "plan_hash": hash, "decision": decision,
            "_meta": {"idempotency_key": new_id().unwrap(), "origin": "local"}
        }))
        .unwrap()
    }

    /// Every event of `stream`, as JSON with its position in the whole log.
    fn stream(desk: &ApprovalDesk, name: &str) -> Vec<(i64, Value)> {
        let stream: EventEnvelopeV1Stream = serde_json::from_value(json!(name)).unwrap();
        desk.with_gates(|log, _| {
            log.log()
                .read_stream(&stream, 1, 100)
                .unwrap()
                .into_iter()
                .map(|stored| {
                    (
                        stored.position.get(),
                        serde_json::to_value(stored.event).unwrap(),
                    )
                })
                .collect()
        })
    }

    const OVER_LIMIT: &str = "approval_item_limit = 5";

    #[test]
    fn the_list_carries_the_plan_record_and_an_approval_is_recorded_before_the_grant() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-grant");
        let desk = bench.desk(vec![gate]);
        let session = session();

        let pending = desk.pending(&session).unwrap().approvals;
        let reply = json!({
            "jsonrpc": "2.0", "id": new_id().unwrap(),
            "result": {"approvals": pending, "more": false}
        });
        checked("urn:ads:command:approval.list.response:2", reply).unwrap();
        let [item] = &pending[..] else {
            panic!("expected one pending approval: {pending:?}");
        };
        let plan = desk.with_gates(|log, _| log.log().load(&action).unwrap().plan.to_json());
        assert_eq!(item["plan"], plan);
        assert_eq!(item["plan"]["quarantined"], true);
        assert!(item["plan"]["destination"].is_string());
        assert_eq!(item["approval_id"], approval.as_str());
        assert_eq!(item["not_approvable"], json!([]));

        let cause = new_id().unwrap();
        desk.answer(
            &session,
            &params(&action, &approval, &hash, "approve"),
            &cause,
        )
        .unwrap();
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        let [(answered_at, record)] = &answers[..] else {
            panic!("expected one answer record: {answers:?}");
        };
        assert_eq!(
            record["actor"],
            json!({"kind": "device", "id": "device-01"})
        );
        assert_eq!(record["cause"], cause.as_str());
        assert_eq!(record["data"]["owner_id"], "owner-01");
        assert_eq!(record["data"]["decision"], "approve");
        assert_eq!(record["data"]["plan_hash"], hash.as_str());
        assert!(record["ts"].is_string());
        let decisions = stream(&desk, &format!("approval/{}", approval.as_str()));
        let (granted_at, granted) = decisions.last().unwrap();
        assert_eq!(granted["type"], "urn:ads:event:approval.granted:2");
        assert_eq!(granted["actor"], record["actor"]);
        assert_eq!(granted["cause"], record["id"]);
        assert!(answered_at < granted_at, "the answer is recorded first");
        assert!(desk.pending(&session).unwrap().approvals.is_empty());
        let state = desk.with_gates(|log, gates| gates[0].inspect(log, &action).unwrap().state);
        assert!(matches!(state, State::Approved { .. }), "{state:?}");
    }

    #[test]
    fn an_answered_approval_is_refused_and_not_recorded_again() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-twice");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let deny = params(&action, &approval, &hash, "deny");
        desk.answer(&session, &deny, &new_id().unwrap()).unwrap();
        for decision in ["approve", "deny"] {
            let again = params(&action, &approval, &hash, decision);
            assert_eq!(
                desk.answer(&session, &again, &new_id().unwrap()),
                Err(ApiError::ApprovalAlreadyAnswered),
                "{decision}"
            );
        }
        assert_eq!(
            stream(&desk, &format!("answer/{}", approval.as_str())).len(),
            1
        );
        let decisions = stream(&desk, &format!("approval/{}", approval.as_str()));
        assert_eq!(decisions.len(), 2, "requested and one denial");
        assert_eq!(decisions[1].1["type"], "urn:ads:event:approval.denied:2");
    }

    #[test]
    fn unknown_ids_are_refused_before_anything_is_written() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-unknown");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let other = Id::new("act-other").unwrap();
        let other_approval = Id::new("appr-other").unwrap();
        for answer in [
            params(&other, &approval, &hash, "approve"),
            params(&action, &other_approval, &hash, "deny"),
        ] {
            assert_eq!(
                desk.answer(&session, &answer, &new_id().unwrap()),
                Err(ApiError::ApprovalNotFound)
            );
        }
        for approval in [&approval, &other_approval] {
            assert!(stream(&desk, &format!("answer/{}", approval.as_str())).is_empty());
        }
        assert_eq!(
            stream(&desk, &format!("approval/{}", approval.as_str())).len(),
            1
        );
        assert_eq!(desk.pending(&session).unwrap().approvals.len(), 1);
    }

    #[test]
    fn an_answer_to_another_plan_is_recorded_and_refused() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, _) = bench.waiting(&gate, "act-other-plan");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let seen = format!("sha256:{}", "0".repeat(64));
        let approve = params(&action, &approval, &seen, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalStale)
        );
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        let [(_, record)] = &answers[..] else {
            panic!("expected the answer record: {answers:?}");
        };
        assert_eq!(record["data"]["plan_hash"], seen.as_str());
        assert_eq!(record["data"]["owner_id"], "owner-01");
        assert_eq!(
            stream(&desk, &format!("approval/{}", approval.as_str())).len(),
            1
        );
        assert_eq!(desk.pending(&session).unwrap().approvals.len(), 1);
    }

    #[test]
    fn an_answer_after_the_validity_is_recorded_and_expires_the_approval() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting_since(&gate, "act-late", 2);
        let desk = bench.desk(vec![gate]);
        let session = session();
        assert_eq!(desk.pending(&session).unwrap().approvals.len(), 1);
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalStale)
        );
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        let [(answered_at, record)] = &answers[..] else {
            panic!("expected the answer record: {answers:?}");
        };
        let decisions = stream(&desk, &format!("approval/{}", approval.as_str()));
        let (expired_at, expired) = decisions.last().unwrap();
        assert_eq!(expired["type"], "urn:ads:event:approval.expired:2");
        assert_eq!(expired["cause"], record["id"]);
        assert!(answered_at < expired_at);
        assert!(desk.pending(&session).unwrap().approvals.is_empty());
        let deny = params(&action, &approval, &hash, "deny");
        assert_eq!(
            desk.answer(&session, &deny, &new_id().unwrap()),
            Err(ApiError::ApprovalAlreadyAnswered)
        );
    }

    #[test]
    fn approving_after_a_policy_change_records_the_answer_and_the_denial() {
        let mut bench = Bench::new();
        let planned_under = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&planned_under, "act-policy");
        // The same session folder, now under a policy that would allow the plan as it is.
        let allowing = bench.gate("approval_item_limit = 500");
        let desk = bench.desk(vec![allowing]);
        let session = session();
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalPolicyChanged)
        );
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        let [(answered_at, record)] = &answers[..] else {
            panic!("expected the answer record: {answers:?}");
        };
        assert_eq!(record["data"]["decision"], "approve");
        let decisions = stream(&desk, &format!("approval/{}", approval.as_str()));
        let (denied_at, denied) = decisions.last().unwrap();
        assert_eq!(denied["type"], "urn:ads:event:approval.denied:2");
        assert_eq!(denied["data"]["reason"], "E_POLICY_CHANGED");
        assert!(answered_at < denied_at);
        let state = desk.with_gates(|log, gates| gates[0].inspect(log, &action).unwrap().state);
        assert_eq!(state, State::ApprovalDenied);
    }

    #[test]
    fn a_disabled_control_refuses_everything() {
        let session = session();
        let id = Id::new("act").unwrap();
        let hash = format!("sha256:{}", "0".repeat(64));
        assert_eq!(
            DisabledApprovals.pending(&session),
            Err(ApiError::ApprovalNotConfigured)
        );
        assert_eq!(
            DisabledApprovals.answer(&session, &params(&id, &id, &hash, "approve"), "cause"),
            Err(ApiError::ApprovalNotConfigured)
        );
    }

    /// The same answer sent again, with the key of the first one.
    fn again(answer: &CommandApprovalAnswerV1Params) -> CommandApprovalAnswerV1Params {
        answer.clone()
    }

    fn execute(desk: &ApprovalDesk, action: &Id) -> Result<(), JournalError> {
        let now = now_ts().unwrap();
        desk.with_gates(|log, gates| {
            gates[0]
                .execute(log, action, daemon_meta(1, &now), daemon_meta(1, &now))
                .map(|_| ())
        })
    }

    fn state(desk: &ApprovalDesk, action: &Id) -> State {
        desk.with_gates(|log, gates| gates[0].inspect(log, action).unwrap().state)
    }

    /// The executor parameters of these tests: removing for good on every system, so that a
    /// purge is rehearsed on Linux too, with `overrides` on top.
    fn limits(overrides: &str) -> ads_adapter_fs::Limits {
        ads_adapter_fs::Limits::from_toml_str(&format!(
            "definitive_delete = \"isolated\"\n{overrides}"
        ))
        .unwrap()
    }

    /// Executes `action`, with the next `seq` of its stream for the start and the outcome.
    fn run(desk: &ApprovalDesk, action: &Id) -> crate::gate::ExecutionOutcome {
        let now = now_ts().unwrap();
        let next = stream(desk, &format!("action/{}", action.as_str())).len() as u64 + 1;
        desk.with_gates(|log, gates| {
            gates[0]
                .execute(
                    log,
                    action,
                    daemon_meta(next, &now),
                    daemon_meta(next + 1, &now),
                )
                .unwrap()
        })
    }

    /// A recursive delete for good of the folder `cache` of the work root, with `seen.txt` in
    /// it, waiting for approval. Returns the action, its approval, its hash and the file.
    fn purge(bench: &mut Bench, gate: &SessionGate, name: &str) -> (Id, Id, String, String) {
        purge_in(bench, gate, name, "cache")
    }

    /// The same, of the folder `folder` of the work root.
    fn purge_in(
        bench: &mut Bench,
        gate: &SessionGate,
        name: &str,
        folder: &str,
    ) -> (Id, Id, String, String) {
        let cache = bench.at(&bench.work.clone(), folder);
        fs::create_dir_all(&cache).unwrap();
        let seen = bench.at(&cache, "seen.txt");
        bench.file(&seen);
        let request = PlanRequest {
            operation: OperationRequest::DeleteRecursive,
            work_root: bench.work.clone(),
            targets: vec![TargetRequest {
                path: cache,
                repo_root: false,
            }],
            estimate: None,
        };
        let (action, approval, hash) = bench.ask(gate, name, request, 0);
        (action, approval, hash, seen)
    }

    fn only(pending: &[Value]) -> &Value {
        let [item] = pending else {
            panic!("expected one pending approval: {pending:?}");
        };
        item
    }

    #[test]
    fn an_entry_added_to_an_approved_purge_is_refused_before_the_disk() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        let (action, approval, hash, seen) = purge(&mut bench, &gate, "act-purge");
        let desk = bench.desk(vec![gate]);
        let session = session();

        let pending = desk.pending(&session).unwrap().approvals;
        let item = only(&pending);
        assert_eq!(item["not_approvable"], json!([]));
        let steps = item["effects"]["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["effect"], "delete_recursive");
        assert_eq!(steps[0]["entries"].as_array().unwrap().len(), 2);
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Ok(false)
        );
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        assert_eq!(answers[0].1["type"], ANSWER);
        assert_eq!(answers[0].1["data"]["effects_hash"], item["effects_hash"]);

        // Something the owner never saw lands in the tree after the approval.
        let unseen = bench.at(&bench.at(&bench.work.clone(), "cache"), "unseen.txt");
        bench.file(&unseen);
        let outcome = run(&desk, &action);
        let failure = outcome.operation.unwrap_err();
        assert_eq!(failure.reason, ads_adapter_fs::Reason::EffectsChanged);
        assert!(failure.report.untouched());
        assert_eq!(outcome.recorded.unwrap().state, State::Failed);
        assert!(Path::new(&seen).is_file() && Path::new(&unseen).is_file());
        let started = stream(&desk, &format!("action/{}", action.as_str()));
        assert!(
            started
                .iter()
                .all(|(_, event)| event["type"] != "urn:ads:event:action.started:1"),
            "no license was issued"
        );
        let failed = &started.last().unwrap().1;
        assert_eq!(failed["data"]["error_code"], "E_FS_EFFECTS_CHANGED");
    }

    #[test]
    fn the_effects_listed_are_the_ones_the_execution_reports() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        let (action, approval, hash, seen) = purge(&mut bench, &gate, "act-same");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let pending = desk.pending(&session).unwrap().approvals;
        let listed = only(&pending).clone();
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Ok(false)
        );

        let outcome = run(&desk, &action);
        let report = outcome.operation.unwrap();
        assert_eq!(outcome.recorded.unwrap().state, State::Executed);
        let steps = listed["effects"]["steps"].as_array().unwrap();
        assert_eq!(report.steps.len(), steps.len());
        for (step, listed) in report.steps.iter().zip(steps) {
            assert_eq!(step.status, ads_adapter_fs::Status::Done);
            assert_eq!(listed["target"], step.target.as_str());
            assert_eq!(listed["effect"], "delete_recursive");
            let entries = listed["entries"].as_array().unwrap();
            assert_eq!(entries.len() as u64, step.facts.items);
            let bytes: u64 = entries.iter().map(|e| e["bytes"].as_u64().unwrap()).sum();
            assert_eq!(bytes, step.facts.bytes);
        }
        assert!(!Path::new(&seen).exists());
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        assert_eq!(answers[0].1["data"]["effects_hash"], listed["effects_hash"]);
        let recorded = stream(&desk, &format!("effects/{}", action.as_str()));
        let [(_, record)] = &recorded[..] else {
            panic!("expected one record of effects: {recorded:?}");
        };
        assert_eq!(record["data"]["effects"], listed["effects"]);
        assert_eq!(record["data"]["effects_hash"], listed["effects_hash"]);
    }

    #[test]
    fn a_policy_changed_after_the_approval_is_recorded_as_such() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        let (action, approval, hash, seen) = purge(&mut bench, &gate, "act-policy-run");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Ok(false)
        );

        // The same folder, now with the quarantine on: the delete would be another plan.
        drop(desk);
        bench.log = Some(bench.reopen());
        let gate = bench.gate("");
        let desk = bench.desk(vec![gate]);
        let next = stream(&desk, &format!("action/{}", action.as_str())).len() as u64 + 1;
        let now = now_ts().unwrap();
        let outcome = desk.with_gates(|log, gates| {
            gates[0]
                .execute(
                    log,
                    &action,
                    daemon_meta(next, &now),
                    daemon_meta(next, &now),
                )
                .map(|_| ())
        });
        assert_eq!(outcome, Err(JournalError::PolicyChanged));
        let events = stream(&desk, &format!("action/{}", action.as_str()));
        assert!(
            events
                .iter()
                .all(|(_, event)| event["type"] != "urn:ads:event:action.started:1")
        );
        assert_eq!(
            events.last().unwrap().1["data"]["error_code"],
            "E_POLICY_CHANGED"
        );
        assert!(Path::new(&seen).is_file());
    }

    #[test]
    fn a_change_after_the_license_is_refused_by_the_executor() {
        let mut bench = Bench::new();
        let unseen = bench.at(&bench.at(&bench.work.clone(), "cache"), "unseen.txt");
        let gate = bench.gate("quarantine_enabled = false").after_license({
            let unseen = unseen.clone();
            move || fs::write(&unseen, "unseen").unwrap()
        });
        bench.made.push(PathBuf::from(&unseen));
        let (action, approval, hash, seen) = purge(&mut bench, &gate, "act-late");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Ok(false)
        );

        // The check before the license passes; the file lands right after it.
        let outcome = run(&desk, &action);
        let failure = outcome.operation.unwrap_err();
        assert_eq!(failure.reason, ads_adapter_fs::Reason::EffectsChanged);
        assert_eq!(failure.stage, ads_adapter_fs::Stage::Checks);
        assert!(failure.report.untouched());
        assert_eq!(outcome.recorded.unwrap().state, State::Failed);
        let events = stream(&desk, &format!("action/{}", action.as_str()));
        assert!(
            events
                .iter()
                .any(|(_, event)| event["type"] == "urn:ads:event:action.started:1"),
            "the license was spent"
        );
        assert!(Path::new(&seen).is_file() && Path::new(&unseen).is_file());
    }

    #[test]
    fn the_list_stops_at_its_ceiling_and_says_there_is_more() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        let mut actions = Vec::new();
        for name in ["act-first", "act-second", "act-third"] {
            let cache = bench.at(&bench.work.clone(), name);
            fs::create_dir_all(&cache).unwrap();
            for at in 0..20 {
                let file = bench.at(&cache, &format!("file-{at:02}.txt"));
                bench.file(&file);
            }
            let request = PlanRequest {
                operation: OperationRequest::DeleteRecursive,
                work_root: bench.work.clone(),
                targets: vec![TargetRequest {
                    path: cache,
                    repo_root: false,
                }],
                estimate: None,
            };
            actions.push(bench.ask(&gate, name, request, 0).0);
        }
        let log = bench.log.take().unwrap();
        let unbounded = desk_with(log, vec![gate], usize::MAX);
        let all = unbounded.pending(&session()).unwrap();
        assert_eq!(all.approvals.len(), 3);
        assert!(!all.more);
        let one = serde_json::to_vec(&all.approvals[0]).unwrap().len();
        let Desk { log, gates, .. } = unbounded.inner.into_inner().unwrap();

        // Room for a bit more than one action: the oldest is listed, with its effects.
        let max = one * 3 / 2 + 256;
        let desk = desk_with(log, gates, max);
        let page = desk.pending(&session()).unwrap();
        assert!(page.more);
        let [item] = &page.approvals[..] else {
            panic!("expected the oldest action only: {:?}", page.approvals);
        };
        assert_eq!(item["action_id"], actions[0].as_str());
        assert!(item["effects"].is_object());
        let reply = json!({"approvals": page.approvals, "more": page.more});
        assert!(serde_json::to_vec(&reply).unwrap().len() <= max);
    }

    #[test]
    fn effects_larger_than_half_the_list_are_not_shown_nor_approvable() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        let (action, approval, hash, _) = purge(&mut bench, &gate, "act-wide");
        let desk = desk_with(bench.log.take().unwrap(), vec![gate], 64);
        let session = session();
        let pending = desk.pending(&session).unwrap().approvals;
        let item = only(&pending);
        assert_eq!(item["effects"], Value::Null);
        assert_eq!(item["effects_hash"], Value::Null);
        assert_eq!(
            item["not_approvable"],
            json!([
                "estimate_unknown",
                "definitive_recursive_delete",
                "effects_not_listed"
            ])
        );
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalNotApprovable)
        );
    }

    #[test]
    fn an_unreadable_record_leaves_the_list_and_the_denial_working() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        // A record this core does not read takes the place of the one the rehearsal writes.
        let log = bench.log.as_mut().unwrap().log();
        let stream_name: EventEnvelopeV1Stream =
            serde_json::from_value(json!("effects/act-odd")).unwrap();
        let odd: EventEnvelopeV1 = serde_json::from_value(json!({
            "id": new_id().unwrap(), "type": "urn:ads:event:effects.recorded:1",
            "ts": now_ts().unwrap(), "stream": stream_name, "seq": 1,
            "actor": {"kind": "daemon", "id": "core"}, "cause": null, "data_class": "internal",
            "data": {"action_id": "act-odd", "effects": {"version": 2}}
        }))
        .unwrap();
        log.append(&odd).unwrap();
        let (odd, odd_approval, odd_hash, _) = purge(&mut bench, &gate, "act-odd");
        let (fine, _, _, _) = purge_in(&mut bench, &gate, "act-fine", "fine");
        let desk = bench.desk(vec![gate]);
        let session = session();

        let pending = desk.pending(&session).unwrap().approvals;
        assert_eq!(pending.len(), 2);
        let item = |id: &Id| {
            pending
                .iter()
                .find(|item| item["action_id"] == id.as_str())
                .unwrap()
                .clone()
        };
        let odd_item = item(&odd);
        assert_eq!(odd_item["effects"], Value::Null);
        assert_eq!(
            odd_item["not_approvable"],
            json!([
                "estimate_unknown",
                "definitive_recursive_delete",
                "effects_unreadable"
            ])
        );
        assert_eq!(item(&fine)["not_approvable"], json!([]));

        let approve = params(&odd, &odd_approval, &odd_hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalNotApprovable)
        );
        let deny = params(&odd, &odd_approval, &odd_hash, "deny");
        assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
        assert_eq!(state(&desk, &odd), State::ApprovalDenied);
    }

    #[test]
    fn a_purge_whose_rehearsal_is_too_large_stays_unapprovable() {
        let mut bench = Bench::new();
        let gate = bench
            .gate("quarantine_enabled = false")
            .with_limits(limits("max_effects_bytes = 64"));
        let (action, approval, hash, seen) = purge(&mut bench, &gate, "act-large");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let pending = desk.pending(&session).unwrap().approvals;
        let item = only(&pending);
        assert_eq!(item["effects"], Value::Null);
        assert_eq!(item["effects_hash"], Value::Null);
        // The refusal of the rehearsal is recorded and shown with its code.
        assert_eq!(item["effects_error"], "E_FS_EFFECTS_TOO_LARGE");
        assert_eq!(
            item["not_approvable"],
            json!([
                "estimate_unknown",
                "definitive_recursive_delete",
                "effects_refused"
            ])
        );
        let refused = stream(&desk, &format!("effects/{}", action.as_str()));
        let [(_, refused)] = &refused[..] else {
            panic!("expected the refusal of the rehearsal: {refused:?}");
        };
        assert_eq!(refused["type"], "urn:ads:event:effects.refused:1");
        assert_eq!(refused["data"]["error_code"], "E_FS_EFFECTS_TOO_LARGE");
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalNotApprovable)
        );
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        assert_eq!(answers[0].1["data"]["effects_hash"], Value::Null);
        assert!(matches!(
            state(&desk, &action),
            State::AwaitingApproval { .. }
        ));
        assert!(matches!(
            execute(&desk, &action),
            Err(JournalError::Transition)
        ));
        assert!(Path::new(&seen).is_file());
        // Denying stays allowed.
        let deny = params(&action, &approval, &hash, "deny");
        assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
        assert_eq!(state(&desk, &action), State::ApprovalDenied);
    }

    #[test]
    fn a_purge_granted_without_a_record_of_effects_is_refused_before_the_disk() {
        let mut bench = Bench::new();
        let gate = bench
            .gate("quarantine_enabled = false")
            .with_limits(limits("max_effects_bytes = 64"));
        let (action, approval, hash, seen) = purge(&mut bench, &gate, "act-bare");
        // Granted another way than the API: no answer names a record of effects.
        let now = now_ts().unwrap();
        let mut meta = daemon_meta(2, &now);
        meta.actor = Actor {
            kind: ActorKind::Device,
            id: Id::new("device-01").unwrap(),
        };
        gate.apply(
            bench.log.as_mut().unwrap(),
            &action,
            Command::Grant {
                action_id: action.clone(),
                approval_id: approval,
                plan_hash: PlanHash::new(hash).unwrap(),
                via: Via::Tray,
            },
            meta,
        )
        .unwrap();
        let desk = bench.desk(vec![gate]);
        let outcome = run(&desk, &action);
        let failure = outcome.operation.unwrap_err();
        assert_eq!(failure.reason, ads_adapter_fs::Reason::EffectsChanged);
        assert_eq!(outcome.recorded.unwrap().state, State::Failed);
        assert!(Path::new(&seen).is_file());
    }

    #[test]
    fn the_reasons_not_to_approve_follow_the_record() {
        let golden = |name: &str| {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../contracts/golden/record.action-plan.v2/valid")
                .join(name);
            let text = fs::read_to_string(path).unwrap();
            serde_json::from_str::<ads_core::gate::ActionPlanRecord>(&text)
                .unwrap()
                .fields()
                .clone()
        };
        // A delete of named targets with an estimate, moved to the quarantine.
        assert!(not_approvable(&golden("05-quarantined.json"), false).is_empty());
        assert_eq!(
            not_approvable(&golden("02-requires-approval.json"), false),
            [
                "estimate_unknown",
                "wildcard_target",
                "definitive_recursive_delete"
            ]
        );
        assert_eq!(
            not_approvable(&golden("01-base.json"), false),
            ["definitive_recursive_delete"]
        );
        let moved = golden("04-move.json");
        assert!(not_approvable(&moved, false).is_empty());
        let mut restore = moved;
        restore.destination = Some(restore.work_root.clone());
        assert_eq!(not_approvable(&restore, false), ["restore_from_manifest"]);
        // A record of the effects a rehearsal found fixes what the plan leaves to the disk,
        // except for a wildcard and a clean of untracked files, which are never rehearsed.
        assert!(not_approvable(&restore, true).is_empty());
        assert!(not_approvable(&golden("01-base.json"), true).is_empty());
        assert_eq!(
            not_approvable(&golden("02-requires-approval.json"), true),
            ["wildcard_target", "definitive_recursive_delete"]
        );
    }

    #[test]
    fn a_repeated_key_records_one_answer_and_keeps_its_refusal() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, _) = bench.waiting(&gate, "act-repeat");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let other_plan = params(
            &action,
            &approval,
            &format!("sha256:{}", "1".repeat(64)),
            "approve",
        );
        for _ in 0..2 {
            assert_eq!(
                desk.answer(&session, &again(&other_plan), &new_id().unwrap()),
                Err(ApiError::ApprovalStale)
            );
        }
        assert_eq!(
            stream(&desk, &format!("answer/{}", approval.as_str())).len(),
            1
        );
        // The same key with another content is a conflict, and records nothing.
        let mut changed = again(&other_plan);
        changed.decision = Decision::Deny;
        assert_eq!(
            desk.answer(&session, &changed, &new_id().unwrap()),
            Err(ApiError::Conflict)
        );
        assert_eq!(
            stream(&desk, &format!("answer/{}", approval.as_str())).len(),
            1
        );
        assert!(matches!(
            state(&desk, &action),
            State::AwaitingApproval { .. }
        ));
    }

    #[test]
    fn a_decision_left_unwritten_is_taken_again_under_the_recorded_answer() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-resume");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let approve = params(&action, &approval, &hash, "approve");
        // The answer is written; the decision after it is not.
        bench.vault.fail_after(1);
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::Storage)
        );
        bench.vault.accept_all();
        assert_eq!(
            stream(&desk, &format!("answer/{}", approval.as_str())).len(),
            1
        );
        assert!(matches!(
            state(&desk, &action),
            State::AwaitingApproval { .. }
        ));

        // After the log is opened again, the same key takes the decision and writes no answer.
        drop(desk);
        bench.log = Some(bench.reopen());
        let gate = bench.gate(OVER_LIMIT);
        let desk = bench.desk(vec![gate]);
        assert_eq!(
            desk.answer(&session, &again(&approve), &new_id().unwrap()),
            Ok(false)
        );
        let answers = stream(&desk, &format!("answer/{}", approval.as_str()));
        assert_eq!(answers.len(), 1);
        let decisions = stream(&desk, &format!("approval/{}", approval.as_str()));
        let (_, granted) = decisions.last().unwrap();
        assert_eq!(granted["type"], "urn:ads:event:approval.granted:2");
        assert_eq!(granted["cause"], answers[0].1["id"]);
        // Once decided, the key gives the same outcome back.
        assert_eq!(
            desk.answer(&session, &again(&approve), &new_id().unwrap()),
            Ok(true)
        );
        assert_eq!(
            stream(&desk, &format!("approval/{}", approval.as_str())).len(),
            2
        );
    }

    #[test]
    fn a_refused_answer_write_leaves_the_gate_untouched() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-no-write");
        let desk = bench.desk(vec![gate]);
        let session = session();
        // Only the store of the answer is refused: a gate acting anyway would succeed.
        bench.vault.refuse_next(1);
        let approve = params(&action, &approval, &hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::Storage)
        );
        bench.vault.accept_all();
        assert!(stream(&desk, &format!("answer/{}", approval.as_str())).is_empty());
        assert_eq!(
            stream(&desk, &format!("approval/{}", approval.as_str())).len(),
            1
        );
        assert!(matches!(
            state(&desk, &action),
            State::AwaitingApproval { .. }
        ));
        assert!(matches!(
            execute(&desk, &action),
            Err(JournalError::Transition)
        ));
    }

    #[test]
    fn an_answer_before_the_last_step_is_recorded_and_left_undecided() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        // Planned and asked two hours ahead of the daemon clock, with no rewind tolerated.
        let (action, approval, hash) = bench.waiting_since(&gate, "act-ahead", -2);
        let desk = bench.desk(vec![gate]);
        let session = session();
        let approve = params(&action, &approval, &hash, "approve");
        for _ in 0..2 {
            assert_eq!(
                desk.answer(&session, &again(&approve), &new_id().unwrap()),
                Err(ApiError::ApprovalOutOfOrder)
            );
        }
        assert_eq!(
            stream(&desk, &format!("answer/{}", approval.as_str())).len(),
            1
        );
        assert_eq!(
            stream(&desk, &format!("approval/{}", approval.as_str())).len(),
            1
        );
        assert_eq!(desk.pending(&session).unwrap().approvals.len(), 1);
        assert!(matches!(
            execute(&desk, &action),
            Err(JournalError::Transition)
        ));
    }

    #[test]
    fn another_approval_id_stays_unknown_after_the_decision() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (granted, granted_approval, granted_hash) = bench.waiting(&gate, "act-granted");
        let (denied, denied_approval, denied_hash) = bench.waiting(&gate, "act-denied");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let first = params(&granted, &granted_approval, &granted_hash, "approve");
        desk.answer(&session, &first, &new_id().unwrap()).unwrap();
        let first = params(&denied, &denied_approval, &denied_hash, "deny");
        desk.answer(&session, &first, &new_id().unwrap()).unwrap();
        let wrong = Id::new("appr-wrong").unwrap();
        for (action, approval, hash) in [
            (&granted, &granted_approval, &granted_hash),
            (&denied, &denied_approval, &denied_hash),
        ] {
            let before = stream(&desk, &format!("approval/{}", approval.as_str())).len();
            let other = params(action, &wrong, hash, "deny");
            assert_eq!(
                desk.answer(&session, &other, &new_id().unwrap()),
                Err(ApiError::ApprovalNotFound)
            );
            let right = params(action, approval, hash, "deny");
            assert_eq!(
                desk.answer(&session, &right, &new_id().unwrap()),
                Err(ApiError::ApprovalAlreadyAnswered)
            );
            assert_eq!(
                stream(&desk, &format!("answer/{}", approval.as_str())).len(),
                1
            );
            assert!(stream(&desk, &format!("answer/{}", wrong.as_str())).is_empty());
            let after = stream(&desk, &format!("approval/{}", approval.as_str())).len();
            assert_eq!(before, after);
        }
    }

    #[test]
    fn requests_of_unserved_folders_never_fill_the_page() {
        let mut bench = Bench::with_storage("read_page_limit = 1");
        let other = bench.other_work();
        let unserved = bench.gate(OVER_LIMIT);
        let served = bench.gate_in(&other, OVER_LIMIT);
        bench.waiting(&unserved, "act-unserved");
        let (action, _, _) = bench.waiting(&served, "act-served");
        let desk = bench.desk(vec![served]);
        let pending = desk.pending(&session()).unwrap().approvals;
        let [item] = &pending[..] else {
            panic!("expected the served request: {pending:?}");
        };
        assert_eq!(item["action_id"], action.as_str());
    }
}
