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
mod tests {
    impl ApprovalDesk {
        /// [`Self::with_gates`], which only fails when the desk stays busy.
        fn with_gates_ok<R>(
            &self,
            act: impl FnOnce(&mut ProductLog, &mut [SessionGate]) -> R,
        ) -> R {
            self.with_gates(act).unwrap()
        }
    }

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
        desk.with_gates_ok(|log, _| {
            log.log()
                .unwrap()
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

    #[test]
    fn an_answer_times_out_while_the_gate_owns_the_desk_or_log() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut bench = Bench::with_storage("busy_timeout_ms = 50");
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-busy");
        let shared = bench.log.as_ref().unwrap().shared_log();
        let desk = bench.desk(vec![gate]);
        let answer = params(&action, &approval, &hash, "approve");
        let cause = new_id().unwrap();

        for hold_desk in [true, false] {
            let (entered, waiting) = mpsc::channel();
            let (release, released) = mpsc::channel();
            std::thread::scope(|scope| {
                let desk = &desk;
                let shared = &shared;
                scope.spawn(move || {
                    let pause = || {
                        entered.send(()).unwrap();
                        let _ = released.recv_timeout(Duration::from_secs(10));
                    };
                    if hold_desk {
                        desk.with_gates(|_, _| pause()).unwrap();
                    } else {
                        let _guard = shared.lock().unwrap();
                        pause();
                    }
                });
                waiting.recv_timeout(Duration::from_secs(10)).unwrap();
                let (done, result) = mpsc::channel();
                let answer = &answer;
                let cause = &cause;
                scope.spawn(move || {
                    done.send(desk.answer(&session(), answer, cause)).unwrap();
                });
                let outcome = result.recv_timeout(Duration::from_secs(2));
                let _ = release.send(());
                assert_eq!(outcome.unwrap(), Err(ApiError::StorageBusy));
            });
        }

        assert_eq!(desk.answer(&session(), &answer, &cause), Ok(false));
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
            "result": {"approvals": pending, "more": false, "unreadable": 0}
        });
        checked("urn:ads:command:approval.list.response:4", reply).unwrap();
        let [item] = &pending[..] else {
            panic!("expected one pending approval: {pending:?}");
        };
        let plan =
            desk.with_gates_ok(|log, _| log.log().unwrap().load(&action).unwrap().plan.to_json());
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
        let state = desk.with_gates_ok(|log, gates| gates[0].inspect(log, &action).unwrap().state);
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

    #[tokio::test]
    async fn approving_after_a_policy_change_returns_a_refusal_and_closes_the_request() {
        use crate::api::tests::{authenticate, fake_service};
        use futures_util::{SinkExt, StreamExt};
        use std::sync::Arc;
        use tokio_tungstenite::tungstenite::Message;

        let mut bench = Bench::new();
        let original = bench.gate(OVER_LIMIT);
        let (changed, approval, hash) = bench.waiting(&original, "act-policy-changed");
        let other = bench.other_work();
        let unchanged_gate = bench.gate_in(&other, OVER_LIMIT);
        let (unchanged, _, _) = bench.waiting(&unchanged_gate, "act-policy-unchanged");
        let replacement = bench.gate("approval_item_limit = 500");
        let desk = Arc::new(bench.desk(vec![replacement, unchanged_gate]));
        let (api, _, _, peer) = fake_service().await;
        let api = api.with_approvals(desk);
        let port = api.port().unwrap();
        let url = api.launch_url().unwrap();
        let (_, token) = url.split_once("&token=").unwrap();
        let server = tokio::spawn(api.run());

        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let mut ws = authenticate(port, token).await;
            let mut exchange = async |method: &str, mut params: Value| {
                let id = new_id().unwrap();
                params["_meta"] = json!({"origin": "local", "idempotency_key": id});
                let command = json!({
                    "jsonrpc": "2.0", "id": id, "method": method, "params": params
                });
                ws.send(Message::Text(command.to_string().into()))
                    .await
                    .unwrap();
                let reply: Value =
                    serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(reply["id"], id);
                reply
            };

            let reply = exchange("approval.list", json!({})).await;
            let result = &reply["result"];
            assert_eq!(result["more"], false);
            assert_eq!(result["unreadable"], 0);
            let [changed_item, unchanged_item] = result["approvals"].as_array().unwrap().as_slice()
            else {
                panic!("expected both pending approvals: {reply}");
            };
            assert_eq!(changed_item["action_id"], changed.as_str());
            assert_eq!(changed_item["not_approvable"], json!(["policy_changed"]));
            assert_eq!(unchanged_item["action_id"], unchanged.as_str());
            assert_eq!(unchanged_item["not_approvable"], json!([]));

            let refused = exchange(
                "approval.answer",
                json!({
                    "action_id": changed, "approval_id": approval,
                    "plan_hash": hash, "decision": "approve"
                }),
            )
            .await;
            assert_eq!(refused["error"]["code"], -32021, "{refused}");

            let remaining = exchange("approval.list", json!({})).await;
            let result = &remaining["result"];
            assert_eq!(result["more"], false);
            assert_eq!(result["unreadable"], 0);
            let approvals = result["approvals"].as_array().unwrap();
            assert_eq!(only(approvals)["action_id"], unchanged.as_str());
        })
        .await
        .expect("approval exchange timed out");
        server.abort();
        peer.abort();
    }

    #[test]
    fn approving_after_a_policy_change_records_the_answer_and_the_denial() {
        let mut bench = Bench::new();
        let planned_under = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&planned_under, "act-policy");
        let (manual, manual_approval, manual_hash) =
            bench.waiting(&planned_under, "act-policy-manual-denial");
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
        let state = desk.with_gates_ok(|log, gates| gates[0].inspect(log, &action).unwrap().state);
        assert_eq!(state, State::ApprovalDenied);

        let deny = params(&manual, &manual_approval, &manual_hash, "deny");
        assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
        assert!(desk.pending(&session).unwrap().approvals.is_empty());
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

    /// Executes `action` with the next `seq` of its stream for the start and the outcome, so
    /// that an error comes from the gate and never from a sequence the journal refuses.
    fn execute(desk: &ApprovalDesk, action: &Id) -> Result<(), JournalError> {
        let now = now_ts().unwrap();
        let next = stream(desk, &format!("action/{}", action.as_str())).len() as u64 + 1;
        desk.with_gates_ok(|log, gates| {
            gates[0]
                .execute(
                    log,
                    action,
                    daemon_meta(next, &now),
                    daemon_meta(next + 1, &now),
                )
                .map(|_| ())
        })
    }

    fn state(desk: &ApprovalDesk, action: &Id) -> State {
        desk.with_gates_ok(|log, gates| gates[0].inspect(log, action).unwrap().state)
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
        desk.with_gates_ok(|log, gates| {
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
        let outcome = desk.with_gates_ok(|log, gates| {
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
        let reply = json!({"approvals": page.approvals, "more": page.more, "unreadable": u64::MAX});
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
        let mut log = bench.log.as_mut().unwrap().log().unwrap();
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
        drop(log);
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
        assert!(
            crate::diagnostic::seen()
                .iter()
                .any(|warning| warning.starts_with("record of effects of act-odd not readable"))
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

    /// A purge of the folder `name` waiting for approval under `gate`, made with `overrides`, whose record of effects holds what a
    /// rehearsal finds but names `hash_of` those effects as its hash. The record is written
    /// first; a scratch log gives its content. Returns what [`purge`] returns and that hash.
    fn purge_recorded_with(
        bench: &mut Bench,
        gate: &SessionGate,
        overrides: &str,
        name: &str,
        hash_of: impl FnOnce(&Effects) -> String,
    ) -> (Id, Id, String, String, String) {
        let scratch_root = bench.root.join(format!("scratch-{name}"));
        let scratch =
            crate::audit::test_product_log(&scratch_root, &bench.storage, &TestVault::default());
        let main = bench.log.replace(scratch).unwrap();
        // A gate over the scratch log: the policy of a gate names the files of its log.
        let scratch_gate = bench.gate(overrides);
        let (_, _, rehearsed_hash, _) = purge_in(bench, &scratch_gate, name, name);
        drop(scratch_gate);
        let scratch = bench.log.replace(main).unwrap();
        let stream_name: EventEnvelopeV1Stream =
            serde_json::from_value(json!(format!("effects/{name}"))).unwrap();
        let read = scratch
            .log()
            .unwrap()
            .read_stream(&stream_name, 1, 2)
            .unwrap();
        let [stored] = &read[..] else {
            panic!("expected the rehearsal record: {read:?}");
        };
        let mut record = serde_json::to_value(&stored.event).unwrap();
        drop(scratch);
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = fs::remove_file(scratch_root.join("audit").join(format!("audit.db{suffix}")));
        }
        assert_eq!(record["type"], "urn:ads:event:effects.recorded:1");
        let effects: Effects = serde_json::from_value(record["data"]["effects"].clone()).unwrap();
        let hash = hash_of(&effects);
        record["id"] = json!(new_id().unwrap());
        record["data"]["effects_hash"] = json!(hash);
        let event: EventEnvelopeV1 = serde_json::from_value(record).unwrap();
        bench
            .log
            .as_mut()
            .unwrap()
            .log()
            .unwrap()
            .append(&event)
            .unwrap();

        let (action, approval, plan_hash, seen) = purge_in(bench, gate, name, name);
        assert_eq!(
            plan_hash, rehearsed_hash,
            "the same plan as the one rehearsed"
        );
        (action, approval, plan_hash, seen, hash)
    }

    #[test]
    fn a_record_hashed_in_another_form_is_unreadable_and_never_executed() {
        const OVERRIDES: &str = "quarantine_enabled = false";
        let mut bench = Bench::new();
        let gate = bench.gate(OVERRIDES);
        let canonical = crate::gate::effects_hash;
        // The bytes the record had before the keys were sorted by hand: the fields in the order
        // of the struct, as a `Value` keeping insertion order would also give.
        let legacy = |effects: &Effects| -> String {
            let bytes = serde_json::to_vec(effects).unwrap();
            format!("sha256:{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes))
        };
        let (same, same_approval, same_hash, same_seen, same_effects) =
            purge_recorded_with(&mut bench, &gate, OVERRIDES, "act-same-form", canonical);
        let (other, other_approval, other_hash, other_seen, _) =
            purge_recorded_with(&mut bench, &gate, OVERRIDES, "act-other-form", |effects| {
                assert_ne!(legacy(effects), canonical(effects));
                legacy(effects)
            });
        let (granted, granted_approval, granted_hash, granted_seen, granted_effects) =
            purge_recorded_with(&mut bench, &gate, OVERRIDES, "act-granted-form", legacy);
        let desk = bench.desk(vec![gate]);
        let session = session();

        let pending = desk.pending(&session).unwrap().approvals;
        let item = |id: &Id| {
            pending
                .iter()
                .find(|item| item["action_id"] == id.as_str())
                .unwrap()
                .clone()
        };
        let same_item = item(&same);
        assert_eq!(same_item["not_approvable"], json!([]));
        assert_eq!(same_item["effects_hash"], same_effects.as_str());
        let other_item = item(&other);
        assert_eq!(other_item["effects"], Value::Null);
        assert_eq!(
            other_item["not_approvable"],
            json!([
                "estimate_unknown",
                "definitive_recursive_delete",
                "effects_unreadable"
            ])
        );

        assert_eq!(
            item(&granted)["not_approvable"],
            other_item["not_approvable"]
        );
        let approve = params(&other, &other_approval, &other_hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Err(ApiError::ApprovalNotApprovable)
        );
        let deny = params(&other, &other_approval, &other_hash, "deny");
        assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
        assert_eq!(state(&desk, &other), State::ApprovalDenied);
        assert!(Path::new(&other_seen).is_file());

        // An approval naming that hash, granted anyway: the execution stops before the license.
        let answer = new_id().unwrap();
        let ts = now_ts().unwrap();
        let answers = format!("answer/{}", granted_approval.as_str());
        let seq = stream(&desk, &answers).len() + 1;
        let value = checked(
            ANSWER,
            json!({
                "id": answer, "type": ANSWER, "ts": ts, "stream": answers, "seq": seq,
                "actor": {"kind": "device", "id": "device-01"}, "cause": new_id().unwrap(),
                "data_class": "internal",
                "data": {
                    "approval_id": granted_approval, "action_id": granted, "plan_hash": granted_hash,
                    "effects_hash": granted_effects, "decision": "approve",
                    "owner_id": "owner-01", "via": "local_ui", "idempotency_key": new_id().unwrap()
                }
            }),
        )
        .unwrap();
        let event: EventEnvelopeV1 = serde_json::from_value(value).unwrap();
        let next = stream(&desk, &format!("action/{}", granted.as_str())).len() as u64 + 1;
        desk.with_gates_ok(|log, gates| {
            log.log().unwrap().append(&event).unwrap();
            let mut meta = daemon_meta(next, &ts);
            meta.actor = Actor {
                kind: ActorKind::Device,
                id: Id::new("device-01").unwrap(),
            };
            meta.cause = Some(Ulid::new(answer).unwrap());
            gates[0]
                .apply(
                    log,
                    &granted,
                    Command::Grant {
                        action_id: granted.clone(),
                        approval_id: granted_approval.clone(),
                        plan_hash: PlanHash::new(granted_hash.clone()).unwrap(),
                        via: Via::LocalUi,
                    },
                    meta,
                )
                .unwrap();
        });
        assert!(matches!(state(&desk, &granted), State::Approved { .. }));
        let outcome = execute(&desk, &granted);
        assert_eq!(outcome, Err(JournalError::Integrity));
        let events = stream(&desk, &format!("action/{}", granted.as_str()));
        assert!(
            events
                .iter()
                .all(|(_, event)| event["type"] != "urn:ads:event:action.started:1"),
            "no license was issued"
        );
        assert!(Path::new(&granted_seen).is_file());

        // The record in the canonical form is approved and executed.
        let approve = params(&same, &same_approval, &same_hash, "approve");
        assert_eq!(
            desk.answer(&session, &approve, &new_id().unwrap()),
            Ok(false)
        );
        let outcome = run(&desk, &same);
        outcome.operation.unwrap();
        assert_eq!(outcome.recorded.unwrap().state, State::Executed);
        assert!(!Path::new(&same_seen).exists());
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
    fn a_fixed_plan_granted_without_a_local_answer_is_refused_before_the_disk() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let seen = bench.at(&bench.work.clone(), "build.log");
        bench.file(&seen);
        fs::write(&seen, "log").unwrap();
        let (action, approval, hash) = bench.waiting_since(&gate, "act-unanswered", 0);
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
        let events = stream(&desk, &format!("action/{}", action.as_str()));
        assert!(
            events
                .iter()
                .all(|(_, event)| event["type"] != "urn:ads:event:action.started:1")
        );
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

    #[test]
    fn the_item_limit_reports_another_served_request() {
        let mut bench = Bench::with_storage("read_page_limit = 1");
        let gate = bench.gate(OVER_LIMIT);
        let other = bench.other_work();
        let unserved = bench.gate_in(&other, OVER_LIMIT);
        let (first, approval, hash) = bench.waiting(&gate, "act-first");
        bench.waiting(&unserved, "act-unserved");
        let (second, _, _) = bench.waiting(&gate, "act-second");
        bench.waiting(&unserved, "act-unserved-last");
        let desk = bench.desk(vec![gate]);
        let session = session();
        let page = desk.pending(&session).unwrap();
        assert_eq!(only(&page.approvals)["action_id"], first.as_str());
        assert!(page.more);

        let deny = params(&first, &approval, &hash, "deny");
        assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
        let page = desk.pending(&session).unwrap();
        assert_eq!(only(&page.approvals)["action_id"], second.as_str());
        assert!(!page.more);
    }

    /// The records of effects this thread asked the log for while `run` ran.
    fn records_read<R>(run: impl FnOnce() -> R) -> (R, usize) {
        let before = crate::gate::RECORDS_READ.with(std::cell::Cell::get);
        let result = run();
        let after = crate::gate::RECORDS_READ.with(std::cell::Cell::get);
        (result, after - before)
    }

    #[test]
    fn the_list_reads_records_up_to_its_ceiling_not_for_every_waiting_action() {
        let mut reads = Vec::new();
        for waiting in [4, 8] {
            let mut bench = Bench::new();
            let gate = bench.gate("quarantine_enabled = false");
            for at in 0..waiting {
                let name = format!("act-{at:02}");
                let folder = format!("cache-{at:02}");
                let path = bench.at(&bench.work, &folder);
                fs::create_dir_all(&path).unwrap();
                for file in 0..64 {
                    bench.file(&bench.at(&path, &format!("entry-{file:02}")));
                }
                purge_in(&mut bench, &gate, &name, &folder);
            }
            let log = bench.log.take().unwrap();
            let unbounded = desk_with(log, vec![gate], usize::MAX);
            let all = unbounded.pending(&session()).unwrap();
            assert_eq!(all.approvals.len(), waiting);
            // The ids of the entries come from the file system and vary in length, so the
            // room is that of the two oldest actions as listed, not twice the first one.
            let two: usize = all.approvals[..2]
                .iter()
                .map(|item| serde_json::to_vec(item).unwrap().len() + 1)
                .sum();
            let Desk { log, gates, .. } = unbounded.inner.into_inner().unwrap();

            // Room for exactly two actions: the third record is never read.
            let desk = desk_with(log, gates, 128 + two);
            let (page, read) = records_read(|| desk.pending(&session()).unwrap());
            assert!(page.more);
            assert_eq!(page.approvals.len(), 2);
            reads.push(read);
        }
        assert_eq!(
            reads[0], reads[1],
            "records read grow with the waiting actions"
        );
        assert_eq!(reads[0], 2);
    }

    #[test]
    fn effects_at_half_the_ceiling_are_listed() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        purge(&mut bench, &gate, "act-half");
        let all = desk_with(bench.log.take().unwrap(), vec![gate], usize::MAX);
        let page = all.pending(&session()).unwrap();
        let effects = &only(&page.approvals)["effects"];
        assert!(effects.is_object());
        let ceiling = 2 * serde_json::to_vec(effects).unwrap().len();
        let Desk { log, gates, .. } = all.inner.into_inner().unwrap();
        let desk = desk_with(log, gates, ceiling);
        let page = desk.pending(&session()).unwrap();
        assert_eq!(&only(&page.approvals)["effects"], effects);
    }

    #[test]
    fn a_record_larger_than_half_the_list_is_never_read() {
        let mut bench = Bench::new();
        let gate = bench.gate("quarantine_enabled = false");
        let (action, approval, hash, _) = purge(&mut bench, &gate, "act-wide");
        let desk = desk_with(bench.log.take().unwrap(), vec![gate], 64);
        let session = session();
        let (pending, read) = records_read(|| desk.pending(&session).unwrap());
        assert_eq!(read, 0);
        assert!(
            only(&pending.approvals)["not_approvable"]
                .as_array()
                .unwrap()
                .contains(&json!("effects_not_listed"))
        );
        let approve = params(&action, &approval, &hash, "approve");
        let (answer, read) = records_read(|| desk.answer(&session, &approve, &new_id().unwrap()));
        assert_eq!(answer, Err(ApiError::ApprovalNotApprovable));
        assert_eq!(read, 0);
    }

    #[test]
    fn an_unreadable_request_does_not_keep_the_list_open() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (broken, _, _) = bench.waiting(&gate, "act-broken");
        // More requests than a history holds: written by no gate of this core.
        {
            let mut log = bench.log.as_mut().unwrap().log().unwrap();
            let requested = log.load(&broken).unwrap().events[1].to_json();
            for extra in 0..4_usize {
                let approval = format!("appr-extra-{extra}");
                let mut json = requested.clone();
                json["id"] = json!(new_id().unwrap());
                json["stream"] = json!(format!("approval/{approval}"));
                json["data"]["approval_id"] = json!(approval);
                let event = Event::from_json(json).unwrap();
                GateJournal::append(&mut *log, &broken, 2 + extra, &event).unwrap();
            }
        }
        let (fine, approval, hash) = bench.waiting(&gate, "act-fine");
        let desk = bench.desk(vec![gate]);
        let page = desk.pending(&session()).unwrap();
        assert_eq!(only(&page.approvals)["action_id"], fine.as_str());
        assert!(!page.more);
        // Left out, but counted, so the owner learns it is there, and which one it is.
        assert_eq!(page.unreadable, 1);
        assert!(
            crate::diagnostic::seen()
                .contains(&"pending approval of act-broken not readable; not listed".to_owned())
        );
        let deny = params(&fine, &approval, &hash, "deny");
        assert_eq!(
            desk.answer(&session(), &deny, &new_id().unwrap()),
            Ok(false)
        );
        let page = desk.pending(&session()).unwrap();
        assert!(page.approvals.is_empty());
        assert!(!page.more);
    }

    /// An action waiting for approval whose history holds more requests than a history does:
    /// four more, written by no gate of this core, asked `hours` ago with a validity of one
    /// hour; `last_expires_at` replaces the deadline of the last one. Returns the action and its
    /// five approvals.
    fn unreadable_since(
        bench: &mut Bench,
        gate: &SessionGate,
        name: &str,
        hours: i64,
        last_expires_at: Option<&str>,
    ) -> (Id, Vec<String>) {
        let (action, approval, _) = bench.waiting_since(gate, name, hours);
        let mut approvals = vec![approval.as_str().to_owned()];
        let mut log = bench.log.as_mut().unwrap().log().unwrap();
        let requested = log.load(&action).unwrap().events[1].to_json();
        for extra in 0..4_usize {
            let approval = format!("appr-{name}-extra-{extra}");
            let mut json = requested.clone();
            json["id"] = json!(new_id().unwrap());
            json["stream"] = json!(format!("approval/{approval}"));
            json["data"]["approval_id"] = json!(approval);
            if let Some(expires_at) = last_expires_at.filter(|_| extra == 3) {
                json["data"]["expires_at"] = json!(expires_at);
            }
            let event = Event::from_json(json).unwrap();
            GateJournal::append(&mut *log, &action, 2 + extra, &event).unwrap();
            approvals.push(approval);
        }
        (action, approvals)
    }

    fn close_params(action: &Id, decision: &str) -> CommandApprovalCloseV1Params {
        serde_json::from_value(json!({
            "action_id": action, "decision": decision,
            "_meta": {"idempotency_key": new_id().unwrap(), "origin": "local"}
        }))
        .unwrap()
    }

    /// The actions the storage still reports as pending with a history that does not read.
    fn unreadable_pending(desk: &ApprovalDesk) -> Vec<String> {
        desk.with_gates_ok(|log, _| {
            let mut seen = Vec::new();
            log.log()
                .unwrap()
                .visit_pending_approvals(100, &mut |entry, _, _| {
                    if let StoredPending::Unreadable(id) = entry {
                        seen.push(id);
                    }
                    Visit::Skip
                })
                .unwrap();
            seen
        })
    }

    /// Every event of the log of these types, as JSON with its position.
    fn of_type(desk: &ApprovalDesk, streams: &[String], kind: &str) -> Vec<(i64, Value)> {
        streams
            .iter()
            .flat_map(|name| stream(desk, name))
            .filter(|(_, event)| event["type"].as_str().is_some_and(|t| t.starts_with(kind)))
            .collect()
    }

    #[test]
    fn closing_an_unreadable_request_records_it_first_and_never_licenses() {
        for decision in ["deny", "expire"] {
            let mut bench = Bench::new();
            let gate = bench.gate(OVER_LIMIT);
            // An expiry needs a validity that is over; a denial does not.
            let hours = if decision == "expire" { 2 } else { 0 };
            let (broken, approvals) =
                unreadable_since(&mut bench, &gate, "act-broken", hours, None);
            let (fine, _, _) = bench.waiting(&gate, "act-fine");
            let desk = bench.desk(vec![gate]);
            let session = session();
            assert_eq!(unreadable_pending(&desk), ["act-broken"]);

            let close = close_params(&broken, decision);
            let cause = new_id().unwrap();
            assert_eq!(desk.close(&session, &close, &cause), Ok(false));
            assert!(unreadable_pending(&desk).is_empty(), "{decision}");
            assert_eq!(
                only(&desk.pending(&session).unwrap().approvals)["action_id"],
                fine.as_str()
            );

            let records = stream(&desk, "closure/act-broken");
            let [(recorded_at, record)] = &records[..] else {
                panic!("one closing record: {records:?}");
            };
            assert_eq!(record["cause"], cause.as_str());
            assert_eq!(
                record["actor"],
                json!({"kind": "device", "id": "device-01"})
            );
            assert_eq!(record["data"]["decision"], decision);
            assert_eq!(record["data"]["approval_ids"], json!(approvals));
            let streams: Vec<String> = approvals
                .iter()
                .map(|approval| format!("approval/{approval}"))
                .collect();
            let kind = match decision {
                "deny" => "urn:ads:event:approval.denied:",
                _ => "urn:ads:event:approval.expired:",
            };
            let decided = of_type(&desk, &streams, kind);
            assert_eq!(decided.len(), approvals.len());
            for (position, event) in &decided {
                assert!(position > recorded_at, "recorded before any decision");
                assert_eq!(event["cause"], record["id"]);
            }
            // No grant and no license: the action never starts.
            assert!(of_type(&desk, &streams, "urn:ads:event:approval.granted:").is_empty());
            assert!(execute(&desk, &broken).is_err());
            let action_stream = vec!["action/act-broken".to_owned()];
            assert!(of_type(&desk, &action_stream, "urn:ads:event:action.started:").is_empty());

            // The same key again records nothing more; a new key finds nothing open.
            assert_eq!(desk.close(&session, &again_close(&close), &cause), Ok(true));
            assert_eq!(stream(&desk, "closure/act-broken").len(), 1);
            assert_eq!(
                desk.close(
                    &session,
                    &close_params(&broken, decision),
                    &new_id().unwrap()
                ),
                Err(ApiError::ApprovalAlreadyAnswered)
            );
            assert_eq!(stream(&desk, "closure/act-broken").len(), 1);
        }
    }

    #[test]
    fn an_unreadable_request_is_never_expired_before_its_deadline() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        // Asked two hours ago with a validity of one hour, but one request is still valid.
        let (broken, approvals) = unreadable_since(
            &mut bench,
            &gate,
            "act-broken",
            2,
            Some("2999-01-01T00:00:00Z"),
        );
        let desk = bench.desk(vec![gate]);
        let session = session();
        let close = close_params(&broken, "expire");
        assert_eq!(
            desk.close(&session, &close, &new_id().unwrap()),
            Err(ApiError::Invalid)
        );
        // Refused before anything is written: no closing record and no decision.
        assert!(stream(&desk, "closure/act-broken").is_empty());
        let streams: Vec<String> = approvals
            .iter()
            .map(|approval| format!("approval/{approval}"))
            .collect();
        assert!(of_type(&desk, &streams, "urn:ads:event:approval.expired:").is_empty());
        assert_eq!(unreadable_pending(&desk), ["act-broken"]);
        // Denying stays allowed before the deadline.
        let deny = close_params(&broken, "deny");
        assert_eq!(desk.close(&session, &deny, &new_id().unwrap()), Ok(false));
        assert!(unreadable_pending(&desk).is_empty());
    }

    #[test]
    fn a_recorded_expiry_resumes_after_the_clock_moves_back() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let deadline = "2099-01-01T00:00:00Z";
        let (action, approvals) =
            unreadable_since(&mut bench, &gate, "act-resume", 2, Some(deadline));
        let desk = bench.desk(vec![gate]);
        let close = close_params(&action, "expire");
        let streams: Vec<_> = approvals
            .iter()
            .map(|approval| format!("approval/{approval}"))
            .collect();
        let expired = "urn:ads:event:approval.expired:2";

        bench.vault.fail_after(2);
        assert_eq!(
            desk.close_unreadable(&close, &new_id().unwrap(), || Ok(deadline.to_owned())),
            Err(ApiError::Storage)
        );
        bench.vault.accept_all();
        let records = stream(&desk, "closure/act-resume");
        assert_eq!(records.len(), 1);
        let record = &records[0].1;
        assert_eq!(record["ts"], deadline);
        assert_eq!(of_type(&desk, &streams, expired).len(), 1);

        drop(desk);
        bench.log = Some(bench.reopen());
        let gate = bench.gate(OVER_LIMIT);
        let desk = bench.desk(vec![gate]);
        let rewound = || Ok("2098-12-31T23:59:59Z".to_owned());
        let new_close = close_params(&action, "expire");
        assert_eq!(
            desk.close_unreadable(&new_close, &new_id().unwrap(), rewound),
            Err(ApiError::Invalid)
        );
        assert_eq!(stream(&desk, "closure/act-resume"), records);
        assert_eq!(of_type(&desk, &streams, expired).len(), 1);
        assert_eq!(
            desk.close_unreadable(&close, &new_id().unwrap(), rewound),
            Ok(true)
        );
        assert_eq!(stream(&desk, "closure/act-resume"), records);
        for approval_stream in streams {
            let decisions = of_type(&desk, &[approval_stream], expired);
            assert_eq!(decisions.len(), 1);
            assert_eq!(decisions[0].1["cause"], record["id"]);
            assert_eq!(decisions[0].1["ts"], deadline);
        }
    }

    #[test]
    fn a_legacy_expiry_resumes_once_the_remaining_requests_are_due() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let recorded_at = "2098-12-31T23:59:58Z";
        let deadline = "2099-01-01T00:00:00Z";
        let resumed_at = "2099-01-01T00:00:01Z";
        let (action, approvals) =
            unreadable_since(&mut bench, &gate, "act-legacy-expiry", 2, Some(deadline));
        let desk = bench.desk(vec![gate]);
        let close = close_params(&action, "expire");
        let closure_stream = "closure/act-legacy-expiry";
        let streams: Vec<_> = approvals
            .iter()
            .map(|approval| format!("approval/{approval}"))
            .collect();
        let expired = "urn:ads:event:approval.expired:2";

        let record = {
            let mut desk = desk.lock().unwrap();
            let Closable::Requests(open) = desk
                .log
                .log()
                .unwrap()
                .closable_requests(&action, MAX_CLOSED)
                .unwrap()
            else {
                panic!("expected unreadable requests");
            };
            let ids: Vec<_> = approvals.iter().map(String::as_str).collect();
            let record = desk
                .record_closing(&close, &ids, recorded_at, &new_id().unwrap())
                .unwrap();
            let first = open
                .iter()
                .find(|request| request.approval_id == approvals[0]);
            desk.close_request(&close, first.unwrap(), &record, recorded_at)
                .unwrap();
            record
        };
        let records = stream(&desk, closure_stream);
        assert_eq!(records.len(), 1);
        let decisions = of_type(&desk, &streams, expired);
        assert_eq!(decisions.len(), 1);

        drop(desk);
        bench.log = Some(bench.reopen());
        let gate = bench.gate(OVER_LIMIT);
        let desk = bench.desk(vec![gate]);
        let retry =
            |at: &str| desk.close_unreadable(&close, &new_id().unwrap(), || Ok(at.to_owned()));
        assert_eq!(retry("2098-12-31T23:59:59Z"), Err(ApiError::Invalid));
        assert_eq!(stream(&desk, closure_stream), records);
        assert_eq!(of_type(&desk, &streams, expired), decisions);

        assert_eq!(retry(resumed_at), Ok(true));
        assert_eq!(retry("2098-12-31T23:59:59Z"), Ok(true));
        assert_eq!(stream(&desk, closure_stream), records);
        for (index, approval_stream) in streams.into_iter().enumerate() {
            let decisions = of_type(&desk, &[approval_stream], expired);
            assert_eq!(decisions.len(), 1);
            assert_eq!(decisions[0].1["cause"], record);
            let expected_ts = if index == 0 { recorded_at } else { resumed_at };
            assert_eq!(decisions[0].1["ts"], expected_ts);
        }
    }

    fn again_close(close: &CommandApprovalCloseV1Params) -> CommandApprovalCloseV1Params {
        close.clone()
    }

    #[test]
    fn a_readable_request_is_closed_only_by_an_answer() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (fine, approval, hash) = bench.waiting(&gate, "act-fine");
        let desk = bench.desk(vec![gate]);
        let session = session();
        for decision in ["deny", "expire"] {
            assert_eq!(
                desk.close(&session, &close_params(&fine, decision), &new_id().unwrap()),
                Err(ApiError::ApprovalReadable)
            );
        }
        assert_eq!(
            desk.close(
                &session,
                &close_params(&Id::new("act-unknown").unwrap(), "deny"),
                &new_id().unwrap()
            ),
            Err(ApiError::ApprovalNotFound)
        );
        assert!(stream(&desk, "closure/act-fine").is_empty());
        assert!(matches!(
            state(&desk, &fine),
            State::AwaitingApproval { .. }
        ));
        // The normal path still answers it.
        let deny = params(&fine, &approval, &hash, "deny");
        assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
        assert_eq!(state(&desk, &fine), State::ApprovalDenied);
    }

    #[test]
    fn a_disabled_control_refuses_a_closing() {
        let close = close_params(&Id::new("act-01").unwrap(), "deny");
        assert_eq!(
            DisabledApprovals.close(&session(), &close, "cause"),
            Err(ApiError::ApprovalNotConfigured)
        );
    }
}
