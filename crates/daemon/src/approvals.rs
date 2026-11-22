//! Owner answers to the action gate for the local API: the approvals the session gates wait
//! for, and each answer recorded in the anchored audit log before the gate acts on it. A
//! request whose history does not read is closed by the owner, never granted.

use std::num::NonZeroU64;
use std::sync::{Mutex, MutexGuard};

use ads_contracts::{
    CommandApprovalAnswerV1Params, CommandApprovalAnswerV1ParamsDecision as Decision,
    CommandApprovalCloseV1Params, CommandApprovalCloseV1ParamsDecision as Closing, CommonIdV1,
    EventEnvelopeV1, EventEnvelopeV1Stream,
};
use ads_core::gate::{
    AbsPath, ActionPlanFields, ActionPlanRecord, Actor, ActorKind, Command, DataClass, Event,
    EventMeta, GateHistory, GateJournal, Id, JournalError, PlanHash, PlanOperation, State,
    Timestamp, Ulid, Via,
};
use serde_json::{Value, json};

use crate::api::{ApiError, checked};
use crate::audit::{ProductLog, lock_with_timeout};
use crate::auth::LocalSession;
use crate::events::{new_id, now_ts};
use crate::gate::{EffectsSource, Recorded, SessionGate, effects_size, recorded_effects};
use ads_adapter_fs::Effects;
use ads_storage::{Closable, Pending as StoredPending, StorageError, Visit};

/// The type of the answer record and the family of its `answer/<approval_id>` stream.
const ANSWER: &str = "urn:ads:event:answer.recorded:2";

/// The most approvals one `approval.list` answers: the `maxItems` of its response contract.
const MAX_LISTED: u32 = 1000;

/// The type of the closing record and the family of its `closure/<action_id>` stream.
const CLOSURE: &str = "urn:ads:event:closure.recorded:1";

/// The most requests one `approval.close` closes: the `maxItems` of `approval_ids` in its
/// record. The rest stay open for another command.
const MAX_CLOSED: u32 = 64;

/// The reason of a denial written by `approval.close`.
const UNREADABLE_REASON: &str = "E_HISTORY_UNREADABLE";

/// What the local API asks of the gate. Each call takes the proof that the connection passed
/// the token check; the owner and the device come from the implementation, never the request.
pub trait ApprovalControl: Send + Sync + 'static {
    /// The actions waiting for the owner, oldest first, as the `result` of
    /// `command.approval.list.response:4`.
    fn pending(&self, session: &LocalSession) -> Result<Pending, ApiError>;
    /// Records the answer, then lets the gate act on it. `cause` is the command id. Returns
    /// whether the outcome is the one of an earlier answer with the same idempotency key.
    fn answer(
        &self,
        session: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<bool, ApiError>;
    /// Closes, denying or expiring, the open requests of an action whose history does not
    /// read: the closing is recorded first, and no request is ever granted. Returns whether
    /// the outcome is the one of an earlier command with the same idempotency key.
    fn close(
        &self,
        session: &LocalSession,
        close: &CommandApprovalCloseV1Params,
        cause: &str,
    ) -> Result<bool, ApiError>;
}

/// One page of `approval.list`: the actions that fit, oldest first, whether the ceiling of
/// bytes left others out, and how many pending requests with an unreadable history the walk
/// passed over (all of them when `more` is false).
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub approvals: Vec<Value>,
    pub more: bool,
    pub unreadable: u64,
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
    fn close(
        &self,
        _: &LocalSession,
        _: &CommandApprovalCloseV1Params,
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
    busy_timeout: std::time::Duration,
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
            busy_timeout: log.busy_timeout(),
            inner: Mutex::new(Desk {
                log,
                gates,
                owner,
                max_list_bytes,
            }),
        }
    }

    pub fn with_gates<R>(
        &self,
        act: impl FnOnce(&mut ProductLog, &mut [SessionGate]) -> R,
    ) -> Result<R, ApiError> {
        let mut desk = self.lock()?;
        let Desk { log, gates, .. } = &mut *desk;
        Ok(act(log, gates))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Desk>, ApiError> {
        lock_with_timeout(&self.inner, self.busy_timeout).map_err(ApiError::from_storage)
    }
}

/// A record of effects is shown, and so can be approved, only when it takes at most half of
/// the page: the rest of the item and the other actions keep room.
fn listable(effects: &Effects, max_list_bytes: usize) -> bool {
    effects.to_bytes().len() <= max_list_bytes / 2
}

fn effects_record_size(plan: &ActionPlanRecord, ciphertext_bytes: usize) -> usize {
    let fields = plan.fields();
    // A valid stored record repeats the action ID and has two SHA-256 strings.
    let envelope = json!({
        "action_id": fields.action_id,
        "plan_hash": fields.plan_hash,
        "effects_hash": fields.plan_hash,
        "effects": null,
    });
    let overhead = envelope.to_string().len() - "null".len() + ads_storage::cipher::DATA_TAG_BYTES;
    ciphertext_bytes.saturating_sub(overhead)
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
    /// The same reading for the listing and the answer, so that an approval never commits to
    /// a record the owner could not see. The stored size can rule out an oversized record
    /// before decryption.
    fn read(
        log: &(impl EffectsSource + ?Sized),
        plan: &ActionPlanRecord,
        size: Result<Option<usize>, JournalError>,
        max_list_bytes: usize,
    ) -> Self {
        let action = &plan.fields().action_id;
        match size {
            Ok(Some(size)) if effects_record_size(plan, size) > max_list_bytes / 2 => {
                Self::NotListed
            }
            Err(error) => Self::of(Err(error), action, max_list_bytes),
            Ok(_) => Self::of(recorded_effects(log, plan), action, max_list_bytes),
        }
    }

    fn of(recorded: Result<Recorded, JournalError>, action: &Id, max_list_bytes: usize) -> Self {
        match recorded {
            Ok(Recorded::Nothing) => Self::Nothing,
            Ok(Recorded::Effects(effects, _)) if !listable(&effects, max_list_bytes) => {
                Self::NotListed
            }
            Ok(Recorded::Effects(effects, hash)) => Self::Effects(effects, hash),
            Ok(Recorded::Refused(code)) => Self::Refused(code),
            Err(error) => {
                crate::diagnostic::warn(format_args!(
                    "record of effects of {} not readable: {error}",
                    action.as_str()
                ));
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

fn closure_stream(action_id: &Id) -> Result<EventEnvelopeV1Stream, ApiError> {
    serde_json::from_value(json!(format!("closure/{}", action_id.as_str())))
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
        let mut log = self.log.log().map_err(ApiError::from_storage)?;
        let seq = log.head(&stream).map_err(ApiError::from_storage)? + 1;
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
        log.append(&event).map_err(ApiError::from_storage)?;
        Ok(id)
    }

    /// Writes the closing of `approvals` as the owner's device, at the next `seq` of
    /// `closure/<action_id>`.
    fn record_closing(
        &mut self,
        close: &CommandApprovalCloseV1Params,
        approvals: &[&str],
        ts: &str,
        cause: &str,
    ) -> Result<String, ApiError> {
        let stream = closure_stream(&close.action_id)?;
        let mut log = self.log.log().map_err(ApiError::from_storage)?;
        let seq = log.head(&stream).map_err(ApiError::from_storage)? + 1;
        let id = new_id()?;
        let value = json!({
            "id": id, "type": CLOSURE, "ts": ts, "stream": stream, "seq": seq,
            "actor": {"kind": "device", "id": self.owner.device_id},
            "cause": cause, "data_class": "internal",
            "data": {
                "action_id": close.action_id, "approval_ids": approvals,
                "decision": close.decision.as_str(),
                "owner_id": self.owner.owner_id, "via": "local_ui",
                "idempotency_key": close.meta.idempotency_key
            }
        });
        let value = checked(CLOSURE, value).map_err(|_| ApiError::Storage)?;
        let event: EventEnvelopeV1 =
            serde_json::from_value(value).map_err(|_| ApiError::Storage)?;
        log.append(&event).map_err(ApiError::from_storage)?;
        Ok(id)
    }

    /// The closing already recorded for this action with the same idempotency key.
    fn closed_with_key(&mut self, action_id: &Id, key: &str) -> Result<Option<Value>, ApiError> {
        let stream = closure_stream(action_id)?;
        let mut log = self.log.log().map_err(ApiError::from_storage)?;
        find_answer(&mut log, &stream, |event| {
            event["data"]["idempotency_key"] == key
        })
    }

    /// The decision of one request, caused by the closing `record`; the storage checks again,
    /// in the transaction of the write, that the history does not read and the request is open.
    fn close_request(
        &mut self,
        close: &CommandApprovalCloseV1Params,
        request: &ads_storage::OpenRequest,
        record: &str,
        ts: &str,
    ) -> Result<(), ApiError> {
        let (kind, mut data) = match close.decision {
            Closing::Deny => (
                "urn:ads:event:approval.denied:2",
                json!({"reason": UNREADABLE_REASON}),
            ),
            Closing::Expire => ("urn:ads:event:approval.expired:2", json!({})),
        };
        data["approval_id"] = json!(request.approval_id);
        data["action_id"] = json!(close.action_id);
        data["plan_hash"] = json!(request.plan_hash);
        let value = json!({
            "id": new_id()?, "type": kind, "ts": ts,
            "stream": format!("approval/{}", request.approval_id), "seq": request.next_seq,
            "actor": {"kind": "device", "id": self.owner.device_id},
            "cause": record, "data_class": "internal", "data": data
        });
        let value = checked(kind, value).map_err(|_| ApiError::Storage)?;
        let event: EventEnvelopeV1 =
            serde_json::from_value(value).map_err(|_| ApiError::Storage)?;
        let mut log = self.log.log().map_err(ApiError::from_storage)?;
        match log.close_unreadable_request(&close.action_id, &event) {
            Ok(_) => Ok(()),
            // Decided, or readable again, since the probe: nothing was written.
            Err(StorageError::Conflict) => Err(ApiError::ApprovalAlreadyAnswered),
            Err(error) => Err(ApiError::from_storage(error)),
        }
    }

    /// The answer already recorded for this approval with the same idempotency key.
    fn recorded_with_key(
        &mut self,
        approval_id: &Id,
        key: &str,
    ) -> Result<Option<Value>, ApiError> {
        let stream = answer_stream(approval_id)?;
        let mut log = self.log.log().map_err(ApiError::from_storage)?;
        find_answer(&mut log, &stream, |event| {
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
        let desk = self.lock()?;
        let max_list_bytes = desk.max_list_bytes;
        let mut page = Pending {
            approvals: Vec::new(),
            more: false,
            unreadable: 0,
        };
        // The envelope of the response, `"more":false` and the count of unreadable requests.
        let mut used = 128;
        let mut failed = None;
        let visited = desk
            .log
            .log()
            .map_err(ApiError::from_storage)?
            .visit_pending_approvals(MAX_LISTED, &mut |entry, reads, full| {
                let history = match entry {
                    StoredPending::History(history) => history,
                    StoredPending::Unreadable(action_id) => {
                        crate::diagnostic::warn(format_args!(
                            "pending approval of {action_id} not readable; not listed"
                        ));
                        page.unreadable += 1;
                        return Visit::Skip;
                    }
                };
                // Only what a gate here can answer; the rest belongs to another session folder
                // and never takes a place in the page.
                let Some(gate_index) = desk.gate_for(&history) else {
                    return Visit::Skip;
                };
                let Some(Event::ApprovalRequested(requested)) = history.events.last() else {
                    return Visit::Skip;
                };
                if full {
                    page.more = true;
                    return Visit::Stop;
                }
                let size = effects_size(reads, &history.plan);
                // A record that cannot fit in what is left waits for a later page, unread.
                if let Ok(Some(size)) = size
                    && !page.approvals.is_empty()
                    && effects_record_size(&history.plan, size) <= max_list_bytes / 2
                    && used + size > max_list_bytes
                {
                    page.more = true;
                    return Visit::Stop;
                }
                let shown = Shown::read(reads, &history.plan, size, max_list_bytes);
                let effects = shown.effects();
                let mut reasons = shown.reasons(history.plan.fields());
                if desk.gates[gate_index].policy_changed(&history) {
                    reasons.push("policy_changed");
                }
                let data = requested.data();
                let item = json!({
                    "action_id": data.action_id.as_str(),
                    "approval_id": data.approval_id.as_str(),
                    "plan_hash": data.plan_hash.as_str(),
                    "risk": data.risk,
                    "summary": data.summary.as_str(),
                    "requested_at": requested.ts().as_str(),
                    "expires_at": data.expires_at.as_str(),
                    "not_approvable": reasons,
                    "plan": history.plan.to_json(),
                    "effects_hash": effects.map(|(_, hash)| hash),
                    "effects": effects.map(|(effects, _)| effects),
                    "effects_error": match &shown {
                        Shown::Refused(code) => Some(code.as_str()),
                        _ => None,
                    },
                });
                let Ok(size) = serde_json::to_vec(&item).map(|bytes| bytes.len() + 1) else {
                    failed = Some(ApiError::Storage);
                    return Visit::Stop;
                };
                // The oldest action is always listed, so that the page never stays empty.
                if !page.approvals.is_empty() && used + size > max_list_bytes {
                    page.more = true;
                    return Visit::Stop;
                }
                used += size;
                page.approvals.push(item);
                Visit::Take
            });
        drop(desk);
        visited.map_err(ApiError::from_storage)?;
        match failed {
            Some(error) => Err(error),
            None => Ok(page),
        }
    }

    fn answer(
        &self,
        _: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<bool, ApiError> {
        let mut guard = self.lock()?;
        let desk = &mut *guard;
        let action_id: Id = answer.action_id.clone();
        let approval_id: Id = answer.approval_id.clone();
        let plan_hash = PlanHash::new(answer.plan_hash.as_str()).map_err(|_| ApiError::Invalid)?;
        let history = match desk
            .log
            .log()
            .map_err(ApiError::from_storage)?
            .load(&action_id)
        {
            Ok(history) => history,
            Err(JournalError::Missing) => return Err(ApiError::ApprovalNotFound),
            Err(JournalError::Busy) => return Err(ApiError::StorageBusy),
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
            Decision::Approve => {
                let log = desk.log.log().map_err(ApiError::from_storage)?;
                let size = effects_size(&*log, &history.plan);
                Shown::read(&*log, &history.plan, size, desk.max_list_bytes)
            }
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

    fn close(
        &self,
        _: &LocalSession,
        close: &CommandApprovalCloseV1Params,
        cause: &str,
    ) -> Result<bool, ApiError> {
        self.close_unreadable(close, cause, now_ts)
    }
}

impl ApprovalDesk {
    fn close_unreadable(
        &self,
        close: &CommandApprovalCloseV1Params,
        cause: &str,
        clock: impl FnOnce() -> Result<String, ApiError>,
    ) -> Result<bool, ApiError> {
        let mut guard = self.lock()?;
        let desk = &mut *guard;
        let action_id: Id = close.action_id.clone();
        let earlier = desk.closed_with_key(&action_id, close.meta.idempotency_key.as_str())?;
        if let Some(event) = &earlier
            && event["data"]["decision"] != close.decision.as_str()
        {
            return Err(ApiError::Conflict);
        }
        // Read only: a refusal here writes nothing.
        let closable = desk
            .log
            .log()
            .map_err(ApiError::from_storage)?
            .closable_requests(&action_id, MAX_CLOSED)
            .map_err(ApiError::from_storage)?;
        let open = match closable {
            Closable::Missing => return Err(ApiError::ApprovalNotFound),
            Closable::Readable => return Err(ApiError::ApprovalReadable),
            Closable::Requests(open) => open,
        };
        let open: Vec<_> = match &earlier {
            // The same key again: only what the recorded closing named and is still open.
            Some(event) => {
                let named = event["data"]["approval_ids"]
                    .as_array()
                    .ok_or(ApiError::Storage)?;
                open.into_iter()
                    .filter(|request| named.iter().any(|id| id == &request.approval_id))
                    .collect()
            }
            None if open.is_empty() => return Err(ApiError::ApprovalAlreadyAnswered),
            None => open,
        };
        let expiry_is_due = |ts: &str| open.iter().all(|request| request.expired_at(ts));
        let ts = match &earlier {
            Some(event) => {
                let recorded_at = event["ts"].as_str().ok_or(ApiError::Storage)?;
                // Older closings could be recorded before the requests were due.
                if close.decision == Closing::Expire && !expiry_is_due(recorded_at) {
                    clock()?
                } else {
                    recorded_at.to_owned()
                }
            }
            None => clock()?,
        };
        // An expiry before a deadline would record one that did not happen: nothing is written.
        if close.decision == Closing::Expire && !expiry_is_due(&ts) {
            return Err(ApiError::Invalid);
        }
        let record = match &earlier {
            Some(event) => event["id"].as_str().ok_or(ApiError::Storage)?.to_owned(),
            None => {
                let ids: Vec<&str> = open
                    .iter()
                    .map(|request| request.approval_id.as_str())
                    .collect();
                desk.record_closing(close, &ids, &ts, cause)?
            }
        };
        for request in &open {
            desk.close_request(close, request, &record, &ts)?;
        }
        Ok(earlier.is_some())
    }
}

fn gate_error(error: JournalError) -> ApiError {
    match error {
        JournalError::Missing => ApiError::ApprovalNotFound,
        JournalError::Busy => ApiError::StorageBusy,
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
#[path = "approvals_tests.rs"]
mod tests;
