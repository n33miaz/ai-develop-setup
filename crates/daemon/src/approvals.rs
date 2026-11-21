//! Owner answers to the action gate for the local API: the approvals the session gates wait
//! for, and each answer recorded in the anchored audit log before the gate acts on it.

use std::num::NonZeroU64;
use std::sync::{Mutex, MutexGuard};

use ads_contracts::{
    CommandApprovalAnswerV1Params, CommandApprovalAnswerV1ParamsDecision as Decision, CommonIdV1,
    EventEnvelopeV1,
};
use ads_core::gate::{
    AbsPath, Actor, ActorKind, Command, DataClass, Event, EventMeta, GateHistory, GateJournal, Id,
    JournalError, PlanHash, State, Timestamp, Ulid, Via,
};
use serde_json::{Value, json};

use crate::api::{ApiError, checked};
use crate::audit::ProductLog;
use crate::auth::LocalSession;
use crate::events::{new_id, now_ts};
use crate::gate::SessionGate;

/// The type of the answer record and the family of its `answer/<approval_id>` stream.
const ANSWER: &str = "urn:ads:event:answer.recorded:1";

/// The most approvals one `approval.list` answers: the `maxItems` of its response contract.
const MAX_LISTED: u32 = 1000;

/// What the local API asks of the gate. Each call takes the proof that the connection passed
/// the token check; the owner and the device come from the implementation, never the request.
pub trait ApprovalControl: Send + Sync + 'static {
    /// The actions waiting for the owner, oldest first, each as an item of
    /// `command.approval.list.response:1`.
    fn pending(&self, session: &LocalSession) -> Result<Vec<Value>, ApiError>;
    /// Records the answer, then lets the gate act on it. `cause` is the command id.
    fn answer(
        &self,
        session: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<(), ApiError>;
}

/// No gate bound to the audit log: every approval command is refused.
pub struct DisabledApprovals;
impl ApprovalControl for DisabledApprovals {
    fn pending(&self, _: &LocalSession) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::ApprovalNotConfigured)
    }
    fn answer(
        &self,
        _: &LocalSession,
        _: &CommandApprovalAnswerV1Params,
        _: &str,
    ) -> Result<(), ApiError> {
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

/// The audit log and the session gates bound to it. Planning, asking for approval and executing
/// go through [`Self::with_gates`], under the same lock as the answers.
pub struct ApprovalDesk {
    inner: Mutex<Desk>,
}
struct Desk {
    log: ProductLog,
    gates: Vec<SessionGate>,
    owner: ApprovalOwner,
}

impl ApprovalDesk {
    pub fn new(log: ProductLog, gates: Vec<SessionGate>, owner: ApprovalOwner) -> Self {
        Self {
            inner: Mutex::new(Desk { log, gates, owner }),
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

impl Desk {
    fn gate_for(&self, history: &GateHistory) -> Option<usize> {
        let root = AbsPath::parse(history.plan.fields().work_root.as_str()).ok()?;
        self.gates.iter().position(|gate| gate.work_root() == &root)
    }

    /// Writes the answer as the owner's device, at the next `seq` of its stream.
    fn record(
        &mut self,
        answer: &CommandApprovalAnswerV1Params,
        ts: &str,
        cause: &str,
    ) -> Result<String, ApiError> {
        let stream = format!("answer/{}", answer.approval_id.as_str());
        let stream_id = serde_json::from_value(json!(stream)).map_err(|_| ApiError::Invalid)?;
        let log = self.log.log();
        let seq = log.head(&stream_id).map_err(|_| ApiError::Storage)? + 1;
        let id = new_id()?;
        let value = json!({
            "id": id, "type": ANSWER, "ts": ts, "stream": stream, "seq": seq,
            "actor": {"kind": "device", "id": self.owner.device_id},
            "cause": cause, "data_class": "internal",
            "data": {
                "approval_id": answer.approval_id, "action_id": answer.action_id,
                "plan_hash": answer.plan_hash, "decision": answer.decision.as_str(),
                "owner_id": self.owner.owner_id, "via": "local_ui"
            }
        });
        let value = checked(ANSWER, value).map_err(|_| ApiError::Storage)?;
        let event: EventEnvelopeV1 =
            serde_json::from_value(value).map_err(|_| ApiError::Storage)?;
        log.append(&event).map_err(|_| ApiError::Storage)?;
        Ok(id)
    }
}

impl ApprovalControl for ApprovalDesk {
    fn pending(&self, _: &LocalSession) -> Result<Vec<Value>, ApiError> {
        let mut desk = self.lock();
        let histories = desk
            .log
            .log()
            .pending_approvals(MAX_LISTED)
            .map_err(|_| ApiError::Storage)?;
        let mut items = Vec::new();
        for history in &histories {
            // Only what a gate here can answer; the rest belongs to another session folder.
            if desk.gate_for(history).is_none() {
                continue;
            }
            let Some(Event::ApprovalRequested(requested)) = history.events.last() else {
                continue;
            };
            let data = requested.data();
            items.push(json!({
                "action_id": data.action_id.as_str(),
                "approval_id": data.approval_id.as_str(),
                "plan_hash": data.plan_hash.as_str(),
                "risk": data.risk,
                "summary": data.summary.as_str(),
                "requested_at": requested.ts().as_str(),
                "expires_at": data.expires_at.as_str(),
                "plan": history.plan.to_json(),
            }));
        }
        Ok(items)
    }

    fn answer(
        &self,
        _: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<(), ApiError> {
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
        let snapshot = desk.gates[index]
            .inspect(&mut desk.log, &action_id)
            .map_err(gate_error)?;
        let ts = now_ts()?;
        let now = Timestamp::new(ts.as_str()).map_err(|_| ApiError::Storage)?;
        let (lapsed, current_hash) = match &snapshot.state {
            State::AwaitingApproval {
                approval_id: waiting,
                expires_at,
                ..
            } => {
                // Another approval id names no approval of this action: nothing to record it under.
                if waiting != &approval_id {
                    return Err(ApiError::ApprovalNotFound);
                }
                (!now.is_before(expires_at), snapshot.plan_hash.clone())
            }
            State::Planned | State::Denied => return Err(ApiError::ApprovalNotFound),
            _ => return Err(ApiError::ApprovalAlreadyAnswered),
        };

        // The answer is durable before the gate acts: a refused write leaves the gate untouched.
        let recorded = desk.record(answer, &ts, cause)?;
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
            Ok(_) => refused.map_or(Ok(()), Err),
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
    use crate::auth::Token;
    use ads_contracts::EventEnvelopeV1Stream;
    use ads_core::gate::{
        Estimate, GatePolicy, HostFacts, OperationRequest, PlanRequest, ShortText, TargetRequest,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    /// A session folder with one file, its product log and the gate policy overrides.
    struct Bench {
        root: PathBuf,
        work: String,
        log: Option<ProductLog>,
    }

    impl Bench {
        fn new() -> Self {
            let mut unique = [0_u8; 16];
            getrandom::fill(&mut unique).unwrap();
            let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
            let root = base.join(format!("approvals-{:032x}", u128::from_be_bytes(unique)));
            fs::create_dir_all(root.join("fence").join("work")).unwrap();
            let resolved = fs::canonicalize(&root).unwrap();
            let resolved = resolved.to_str().unwrap();
            let root = PathBuf::from(resolved.strip_prefix(r"\\?\").unwrap_or(resolved));
            let work_dir = root.join("fence").join("work");
            fs::write(work_dir.join("build.log"), "log").unwrap();
            let log = crate::audit::test_product_log(&root);
            Self {
                work: work_dir.to_str().unwrap().to_owned(),
                root,
                log: Some(log),
            }
        }

        fn fence(&self) -> PathBuf {
            self.root.join("fence")
        }

        fn gate(&self, overrides: &str) -> SessionGate {
            let home = self.fence().join("fake-home");
            let policy = GatePolicy::from_toml_str(
                overrides,
                &HostFacts {
                    user_home: home.to_str().unwrap(),
                },
            )
            .unwrap();
            let root = AbsPath::parse(&self.work).unwrap();
            SessionGate::new(
                root.clone(),
                vec![root],
                policy,
                self.log.as_ref().unwrap(),
                &self.fence(),
            )
            .unwrap()
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
            let sep = std::path::MAIN_SEPARATOR;
            let request = PlanRequest {
                operation: OperationRequest::Delete,
                work_root: self.work.clone(),
                targets: vec![TargetRequest {
                    path: format!("{}{sep}build.log", self.work),
                    repo_root: false,
                }],
                estimate: Some(Estimate {
                    items: 10,
                    bytes: 3,
                }),
            };
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
                        summary: ShortText::new("Remove the build log").unwrap(),
                        expires_at: Timestamp::new(expires).unwrap(),
                    },
                    daemon_meta(1, &asked_at),
                )
                .unwrap();
            (action_id, approval_id, asked.plan_hash.as_str().to_owned())
        }

        fn desk(&mut self, gates: Vec<SessionGate>) -> ApprovalDesk {
            ApprovalDesk::new(
                self.log.take().unwrap(),
                gates,
                ApprovalOwner {
                    owner_id: Id::new("owner-01").unwrap(),
                    device_id: Id::new("device-01").unwrap(),
                },
            )
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            drop(self.log.take());
            // Only the files this bench made, one by one; the folders stay in `.test-tmp`.
            let _ = fs::remove_file(Path::new(&self.work).join("build.log"));
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
    fn the_list_carries_the_exact_plan_and_an_approval_is_recorded_before_the_grant() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting(&gate, "act-grant");
        let desk = bench.desk(vec![gate]);
        let session = session();

        let pending = desk.pending(&session).unwrap();
        let reply =
            json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "result": {"approvals": pending}});
        checked("urn:ads:command:approval.list.response:1", reply).unwrap();
        let [item] = &pending[..] else {
            panic!("expected one pending approval: {pending:?}");
        };
        let plan = desk.with_gates(|log, _| log.log().load(&action).unwrap().plan.to_json());
        assert_eq!(item["plan"], plan);
        assert_eq!(item["plan"]["quarantined"], true);
        assert!(item["plan"]["destination"].is_string());
        assert_eq!(item["approval_id"], approval.as_str());

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
        assert!(desk.pending(&session).unwrap().is_empty());
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
        assert_eq!(desk.pending(&session).unwrap().len(), 1);
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
        assert_eq!(desk.pending(&session).unwrap().len(), 1);
    }

    #[test]
    fn an_answer_after_the_validity_is_recorded_and_expires_the_approval() {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        let (action, approval, hash) = bench.waiting_since(&gate, "act-late", 2);
        let desk = bench.desk(vec![gate]);
        let session = session();
        assert_eq!(desk.pending(&session).unwrap().len(), 1);
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
        assert!(desk.pending(&session).unwrap().is_empty());
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
}
