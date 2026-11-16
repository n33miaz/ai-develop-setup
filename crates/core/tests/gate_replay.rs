//! The state of an action rebuilt from its events alone: on every path of the state machine,
//! at every step, the log gives the state the machine holds in memory. And the logs the
//! machine would never write are refused.

mod common;

use std::collections::BTreeSet;

use ads_core::gate::{
    Action, ActionPlan, ActionSnapshot, Command, Decision, ErrorCode, Event, ExecutionPermit,
    OperationRequest as Op, ReplayError, ShortText, State, Verdict, Via, classify, replay,
};
use common::{WIN_HOME, WIN_ROOT, at, id, meta, policy, request, win};
use serde_json::Value;

const T0: &str = "2030-01-02T03:04:05Z";
const T1: &str = "2030-01-02T03:06:05Z";
const EXPIRES: &str = "2030-01-02T03:19:05Z";
const AFTER: &str = "2030-01-02T03:19:05.000001Z";

const BUILD: &str = "C:\\Users\\user-a\\work\\proj\\build";

/// The family of a stream and the field of `data` that names its entity, as in the table of
/// `contracts/README.md` ("Stream e entidade"), for the families the gate writes.
const STREAM_ID_FIELD: &[(&str, &str)] = &[("action", "action_id"), ("approval", "approval_id")];

fn allowed() -> Verdict {
    let verdict = win(Op::Delete, &[BUILD]);
    assert_eq!(verdict.decision(), Decision::Allowed);
    verdict
}

fn approval_needed(targets: &[&str]) -> Verdict {
    let mut request = request(Op::DeleteRecursive, WIN_ROOT, targets);
    request.estimate = None;
    let verdict = classify(
        &ActionPlan::normalize(&request).unwrap(),
        &policy(WIN_HOME, ""),
    );
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
    verdict
}

fn denied() -> Verdict {
    let verdict = win(Op::DeleteRecursive, &[WIN_HOME]);
    assert_eq!(verdict.decision(), Decision::Denied);
    verdict
}

/// The rule of the contracts: the stream is `<family>/<id>`, and the id is the one `data`
/// names. Checked on the JSON, apart from the types that built the event.
fn stream_names_the_entity_in_data(event: &Value) {
    let stream = event["stream"].as_str().unwrap();
    let (family, entity) = stream.split_once('/').unwrap();
    let field = STREAM_ID_FIELD
        .iter()
        .find(|(name, _)| *name == family)
        .unwrap_or_else(|| panic!("{stream}: a family the gate does not write"))
        .1;
    assert_eq!(event["data"][field], entity, "{event}");
    let kind = event["type"].as_str().unwrap();
    assert!(
        kind.starts_with(&format!("urn:ads:event:{family}.")),
        "{kind} in stream {stream}"
    );
}

/// Everything that can be asked of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    Authorize,
    Ask,
    Grant,
    Deny,
    Expire,
    Executed,
    Fail,
}

const MOVES: [Move; 7] = [
    Move::Authorize,
    Move::Ask,
    Move::Grant,
    Move::Deny,
    Move::Expire,
    Move::Executed,
    Move::Fail,
];

/// An action taken from its verdict through `moves`, with its permit, if one is out, and its
/// log. `None` when the machine refuses one of the moves. An action cannot be copied, so each
/// branch of the walk is played again from the start.
fn run(verdict: &Verdict, moves: &[Move]) -> Option<(Action, Option<ExecutionPermit>, Vec<Event>)> {
    let (mut action, first, _) = Action::plan(id("act-01"), verdict, meta(T0, 1)).unwrap();
    let mut permit = None;
    let mut log = vec![first];
    for step in moves {
        // `seq` counts in the stream of the event to come.
        let seq =
            |stream: &str| 1 + log.iter().filter(|event| event.stream() == stream).count() as u64;
        let (action_seq, approval_seq) = (seq("action/act-01"), seq("approval/appr-01"));
        let decision = |ts: &str| (meta(ts, approval_seq), id("act-01"), id("appr-01"));
        let plan_hash = action.plan_hash().clone();
        let result = match step {
            Move::Authorize => match action.authorize(verdict.plan(), meta(T1, action_seq)) {
                Ok((next, handed, event)) => {
                    permit = Some(handed);
                    Ok((next, event))
                }
                Err(refusal) => Err(refusal),
            },
            Move::Ask => {
                let command = Command::RequestApproval {
                    approval_id: id("appr-01"),
                    summary: ShortText::new("Apagar a pasta de build").unwrap(),
                    expires_at: at(EXPIRES),
                };
                action.apply(command, meta(T0, approval_seq))
            }
            Move::Grant => {
                let (meta, action_id, approval_id) = decision(T1);
                let command = Command::Grant {
                    action_id,
                    approval_id,
                    plan_hash,
                    via: Via::LocalUi,
                };
                action.apply(command, meta)
            }
            Move::Deny => {
                let (meta, action_id, approval_id) = decision(T1);
                let command = Command::Deny {
                    action_id,
                    approval_id,
                    plan_hash,
                    reason: None,
                };
                action.apply(command, meta)
            }
            Move::Expire => {
                let (meta, action_id, approval_id) = decision(AFTER);
                let command = Command::Expire {
                    action_id,
                    approval_id,
                    plan_hash,
                };
                action.apply(command, meta)
            }
            Move::Executed => {
                let permit = permit.take()?;
                action.apply(Command::Executed { permit }, meta(T1, action_seq))
            }
            Move::Fail => {
                let error_code = ErrorCode::new("E_PLAN_CHANGED").unwrap();
                action.apply(Command::Fail { error_code }, meta(T1, action_seq))
            }
        };
        let (next, event) = result.ok()?;
        action = next;
        log.push(event);
    }
    Some((action, permit, log))
}

/// Walks every path of the machine from one verdict, trying every move at every state. At
/// each step the log so far must rebuild the state in memory.
struct Walk {
    verdict: Verdict,
    paths: Vec<String>,
    final_states: BTreeSet<String>,
    events: Vec<Value>,
}

impl Walk {
    fn step(&mut self, moves: &[Move]) {
        let (action, _, log) = run(&self.verdict, moves).unwrap();

        // Straight from memory, and from the log as it would be read back from storage.
        assert_eq!(replay(&log), Ok(action.snapshot()), "{log:?}");
        let read_back: Vec<Event> = log
            .iter()
            .map(|event| Event::from_json(event.to_json()).unwrap())
            .collect();
        assert_eq!(read_back, log);
        assert_eq!(replay(&read_back), Ok(action.snapshot()));

        let last = log.last().unwrap().to_json();
        stream_names_the_entity_in_data(&last);
        let earlier = log.iter().filter(|event| event.stream() == last["stream"]);
        assert_eq!(last["seq"], earlier.count(), "{last}");
        self.events.push(last);

        let mut moved = false;
        for next in MOVES {
            let mut longer = moves.to_vec();
            longer.push(next);
            if run(&self.verdict, &longer).is_some() {
                moved = true;
                self.step(&longer);
            }
        }
        assert_eq!(moved, !action.state().is_terminal(), "{:?}", action.state());
        if !moved {
            self.final_states.insert(format!("{:?}", action.state()));
            let kinds: Vec<&str> = log
                .iter()
                .map(|event| event.kind().rsplit(':').nth(1).unwrap())
                .collect();
            self.paths.push(kinds.join(" > "));
        }
    }
}

fn walk(verdict: Verdict) -> Walk {
    let mut walk = Walk {
        verdict,
        paths: Vec::new(),
        final_states: BTreeSet::new(),
        events: Vec::new(),
    };
    walk.step(&[]);
    walk.paths.sort();
    walk
}

#[test]
fn replay_gives_the_state_in_memory_on_every_path_of_the_machine() {
    let allowed = walk(allowed());
    assert_eq!(
        allowed.paths,
        [
            "action.planned > action.failed",
            "action.planned > action.started > action.executed",
            "action.planned > action.started > action.failed",
        ]
    );
    let approval = walk(approval_needed(&[BUILD]));
    assert_eq!(
        approval.paths,
        [
            "action.planned > action.failed",
            "action.planned > approval.requested > approval.denied",
            "action.planned > approval.requested > approval.expired",
            "action.planned > approval.requested > approval.granted > action.failed",
            "action.planned > approval.requested > approval.granted > action.started > action.executed",
            "action.planned > approval.requested > approval.granted > action.started > action.failed",
        ]
    );
    let denied = walk(denied());
    assert_eq!(denied.paths, ["action.denied"]);

    // Between them the three walks end in every final state and write every event of the gate.
    let walks = [&allowed, &approval, &denied];
    let final_states: BTreeSet<&str> = walks
        .iter()
        .flat_map(|walk| walk.final_states.iter().map(String::as_str))
        .collect();
    assert_eq!(
        final_states,
        BTreeSet::from([
            "ApprovalDenied",
            "ApprovalExpired",
            "Denied",
            "Executed",
            "Failed"
        ])
    );
    let kinds: BTreeSet<&str> = walks
        .iter()
        .flat_map(|walk| walk.events.iter())
        .map(|event| event["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from([
            "urn:ads:event:action.denied:1",
            "urn:ads:event:action.executed:1",
            "urn:ads:event:action.failed:1",
            "urn:ads:event:action.planned:1",
            "urn:ads:event:action.started:1",
            "urn:ads:event:approval.denied:2",
            "urn:ads:event:approval.expired:2",
            "urn:ads:event:approval.granted:2",
            "urn:ads:event:approval.requested:2",
        ])
    );
}

#[test]
fn every_emitted_event_is_in_the_stream_of_the_entity_in_its_data() {
    let mut checked = 0;
    for verdict in [allowed(), approval_needed(&[BUILD]), denied()] {
        for event in walk(verdict).events {
            stream_names_the_entity_in_data(&event);
            checked += 1;
        }
    }
    assert!(checked >= 16, "only {checked} events checked");

    // The approval events sit in the stream of the approval; the request and each decision
    // name the action they belong to.
    let (_, log) = approved_run("act-01", "appr-01", &approval_needed(&[BUILD]));
    let streams: Vec<&str> = log.iter().map(Event::stream).collect();
    assert_eq!(
        streams,
        [
            "action/act-01",
            "approval/appr-01",
            "approval/appr-01",
            "action/act-01",
            "action/act-01"
        ]
    );
    assert_eq!(log[1].to_json()["data"]["action_id"], "act-01");
    assert_eq!(log[2].to_json()["data"]["action_id"], "act-01");
    assert_eq!(log[3].to_json()["data"]["approval_id"], "appr-01");
}

fn ask(approval_id: &str) -> Command {
    Command::RequestApproval {
        approval_id: id(approval_id),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at: at(EXPIRES),
    }
}

/// The longest path: planned, approval requested and granted, started, executed.
fn approved_run(action_id: &str, approval_id: &str, verdict: &Verdict) -> (Action, Vec<Event>) {
    let (action, planned, _) = Action::plan(id(action_id), verdict, meta(T0, 1)).unwrap();
    let (action, requested) = action.apply(ask(approval_id), meta(T0, 1)).unwrap();
    let grant = Command::Grant {
        action_id: id(action_id),
        approval_id: id(approval_id),
        plan_hash: action.plan_hash().clone(),
        via: Via::Tray,
    };
    let (action, granted) = action.apply(grant, meta(T1, 2)).unwrap();
    let (action, permit, started) = action.authorize(verdict.plan(), meta(T1, 2)).unwrap();
    let (action, executed) = action
        .apply(Command::Executed { permit }, meta(T1, 3))
        .unwrap();
    (action, vec![planned, requested, granted, started, executed])
}

/// The log with the event at `index` rewritten as JSON and read back.
fn rewritten(log: &[Event], index: usize, change: &dyn Fn(&mut Value)) -> Vec<Event> {
    let mut log = log.to_vec();
    let mut json = log[index].to_json();
    change(&mut json);
    log[index] = Event::from_json(json).unwrap();
    log
}

fn unexpected(result: Result<ActionSnapshot, ReplayError>) -> bool {
    matches!(result, Err(ReplayError::UnexpectedEvent { .. }))
}

#[test]
fn logs_the_machine_would_not_write_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    let (action, log) = approved_run("act-01", "appr-01", &verdict);
    assert_eq!(replay(&log), Ok(action.snapshot()));
    assert_eq!(action.state(), &State::Executed);

    assert_eq!(replay(&[]), Err(ReplayError::EmptyLog));
    // Any event but `action.planned` or `action.denied` first.
    for start in 1..log.len() {
        assert_eq!(
            replay(&log[start..]),
            Err(ReplayError::UnexpectedEvent {
                state: None,
                event: log[start].kind().to_owned()
            })
        );
    }
    // A jump: any single event left out of the middle.
    for skipped in 1..log.len() - 1 {
        let mut jumped = log.clone();
        jumped.remove(skipped);
        assert!(replay(&jumped).is_err(), "without event {skipped}");
    }
    // Started with no approval asked, with the approval still pending, and executed with no
    // start.
    assert_eq!(
        replay(&[log[0].clone(), log[3].clone()]),
        Err(ReplayError::ApprovalRequired)
    );
    assert_eq!(
        replay(&[log[0].clone(), log[1].clone(), log[3].clone()]),
        Err(ReplayError::ApprovalRequired)
    );
    assert!(unexpected(replay(&[
        log[0].clone(),
        log[1].clone(),
        log[2].clone(),
        log[4].clone()
    ])));
    // Anything after a final state, and any event twice: a second `action.started` is a
    // second permit, a second `action.executed` a permit used twice.
    for again in &log {
        let mut longer = log.clone();
        longer.push(again.clone());
        assert!(unexpected(replay(&longer)), "{}", again.kind());
    }
    for twice in 0..log.len() {
        let mut repeated = log[..=twice].to_vec();
        repeated.push(log[twice].clone());
        assert!(unexpected(replay(&repeated)), "event {twice} twice");
    }
}

#[test]
fn events_of_another_action_plan_or_approval_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    let (_, log) = approved_run("act-01", "appr-01", &verdict);
    let splice = |index: usize, other: &[Event]| {
        let mut log = log.clone();
        log[index] = other[index].clone();
        replay(&log[..=index])
    };

    let (_, other_action) = approved_run("act-02", "appr-01", &verdict);
    // Every event after the first names its action, the grant included.
    for index in 1..log.len() {
        assert_eq!(
            splice(index, &other_action),
            Err(ReplayError::ActionMismatch),
            "event {index}"
        );
    }

    let other_verdict = approval_needed(&[BUILD, "C:\\Users\\user-a\\work\\proj\\src"]);
    let (_, other_plan) = approved_run("act-01", "appr-01", &other_verdict);
    for index in 1..log.len() {
        assert_eq!(
            splice(index, &other_plan),
            Err(ReplayError::PlanHashMismatch),
            "event {index}"
        );
    }

    let (_, other_approval) = approved_run("act-01", "appr-02", &verdict);
    for index in 2..log.len() {
        assert_eq!(
            splice(index, &other_approval),
            Err(ReplayError::ApprovalMismatch),
            "event {index}"
        );
    }

    // An allowed plan started with an approval it never asked for.
    let allowed = allowed();
    let (action, planned, _) = Action::plan(id("act-01"), &allowed, meta(T0, 1)).unwrap();
    let (_, _, started) = action.authorize(allowed.plan(), meta(T1, 2)).unwrap();
    let with_approval = rewritten(&[planned, started], 1, &|event| {
        event["data"]["approval_id"] = "appr-01".into();
    });
    assert_eq!(replay(&with_approval), Err(ReplayError::ApprovalMismatch));
}

/// Planned and asked, then closed by `decision`: the log up to that decision.
fn decided_run(action_id: &str, verdict: &Verdict, decision: Move) -> (Action, Vec<Event>) {
    let (action, planned, _) = Action::plan(id(action_id), verdict, meta(T0, 1)).unwrap();
    let (action, requested) = action.apply(ask("appr-01"), meta(T0, 1)).unwrap();
    let (action_id, approval_id) = (id(action_id), id("appr-01"));
    let plan_hash = action.plan_hash().clone();
    let (command, ts) = match decision {
        Move::Grant => (
            Command::Grant {
                action_id,
                approval_id,
                plan_hash,
                via: Via::LocalUi,
            },
            T1,
        ),
        Move::Deny => (
            Command::Deny {
                action_id,
                approval_id,
                plan_hash,
                reason: None,
            },
            T1,
        ),
        Move::Expire => (
            Command::Expire {
                action_id,
                approval_id,
                plan_hash,
            },
            AFTER,
        ),
        other => panic!("{other:?} is not a decision of an approval"),
    };
    let (action, decided) = action.apply(command, meta(ts, 2)).unwrap();
    (action, vec![planned, requested, decided])
}

/// Two actions with the same plan and the same approval id: before `action_id` went into the
/// decisions, the grant, the denial and the expiry of one read as those of the other.
#[test]
fn decision_of_an_approval_taken_for_another_action_is_refused() {
    let verdict = approval_needed(&[BUILD]);
    for decision in [Move::Grant, Move::Deny, Move::Expire] {
        let (first, log) = decided_run("act-01", &verdict, decision);
        let (second, other) = decided_run("act-02", &verdict, decision);
        assert_eq!(first.plan_hash(), second.plan_hash());
        assert_eq!(replay(&log), Ok(first.snapshot()));
        assert_eq!(replay(&other), Ok(second.snapshot()));

        // The two decisions differ in nothing but the action they name.
        let (mine, theirs) = (log[2].to_json(), other[2].to_json());
        assert_eq!(mine["data"]["action_id"], "act-01");
        assert_eq!(theirs["data"]["action_id"], "act-02");
        let mut renamed = theirs.clone();
        renamed["data"]["action_id"] = "act-01".into();
        assert_eq!(renamed, mine, "{decision:?}");

        // Swapped, each log is refused, straight from memory and read back from storage.
        for (own, foreign) in [(&log, &other), (&other, &log)] {
            let mut swapped = own.clone();
            swapped[2] = foreign[2].clone();
            assert_eq!(
                replay(&swapped),
                Err(ReplayError::ActionMismatch),
                "{decision:?}"
            );
            let read_back: Vec<Event> = swapped
                .iter()
                .map(|event| Event::from_json(event.to_json()).unwrap())
                .collect();
            assert_eq!(replay(&read_back), Err(ReplayError::ActionMismatch));
        }
    }
}

/// Version 1 of the decisions does not name the action: it does not read as an event of the
/// gate, so no state is rebuilt from it.
#[test]
fn version_1_of_the_approval_decisions_is_refused() {
    let verdict = approval_needed(&[BUILD]);
    for decision in [Move::Grant, Move::Deny, Move::Expire] {
        let (_, log) = decided_run("act-01", &verdict, decision);
        let current = log[2].to_json();
        let kind = current["type"].as_str().unwrap();
        assert!(kind.ends_with(":2"), "{kind}");
        assert!(Event::from_json(current.clone()).is_ok());

        // Exactly what version 1 of the contract holds.
        let mut old = current.clone();
        old["type"] = format!("{}1", kind.strip_suffix('2').unwrap()).into();
        old["data"].as_object_mut().unwrap().remove("action_id");
        assert!(Event::from_json(old).is_err(), "{kind}");
        // Nor the old type with the new field, nor the new type without it.
        let mut mixed = current.clone();
        mixed["type"] = format!("{}1", kind.strip_suffix('2').unwrap()).into();
        assert!(Event::from_json(mixed).is_err(), "{kind}");
        let mut bare = current.clone();
        bare["data"].as_object_mut().unwrap().remove("action_id");
        assert!(Event::from_json(bare).is_err(), "{kind}");
    }
}

#[test]
fn events_dated_before_the_step_they_follow_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    // Asked at T0, granted and started at T1.
    let (_, log) = approved_run("act-01", "appr-01", &verdict);
    let at_instant = |index: usize, ts: &'static str| {
        let log = rewritten(&log, index, &|event| event["ts"] = ts.into());
        replay(&log[..=index])
    };
    let before_request = ["2030-01-02T03:04:04.999999Z", "2020-01-01T00:00:00Z"];
    for early in before_request {
        assert_eq!(at_instant(2, early), Err(ReplayError::OutOfOrder));
    }
    // Started after the request and before the grant, or before both.
    for early in [T0, "2030-01-02T03:06:04.999999Z", "2020-01-01T00:00:00Z"] {
        assert_eq!(at_instant(3, early), Err(ReplayError::OutOfOrder));
    }
    // The same instant is in order, in any spelling of it.
    for same in [T0, "2030-01-02T03:04:05.000000Z"] {
        assert!(at_instant(2, same).is_ok(), "{same}");
    }
    for same in [T1, "2030-01-02T03:06:05.0Z"] {
        assert!(at_instant(3, same).is_ok(), "{same}");
    }
    // Granted at the instant it was asked and started at that same instant.
    let all_at_once = rewritten(&log, 2, &|event| event["ts"] = T0.into());
    let all_at_once = rewritten(&all_at_once, 3, &|event| event["ts"] = T0.into());
    assert!(replay(&all_at_once[..4]).is_ok());

    // The example of the review: the whole approval used with a clock ten years behind.
    let stale = rewritten(&log, 2, &|event| {
        event["ts"] = "2020-01-01T00:00:00Z".into();
    });
    let stale = rewritten(&stale, 3, &|event| {
        event["ts"] = "2020-01-01T00:00:00Z".into();
    });
    assert_eq!(replay(&stale), Err(ReplayError::OutOfOrder));

    // A denial dated before the request.
    let (_, denied) = decided_run("act-01", &verdict, Move::Deny);
    assert!(replay(&denied).is_ok());
    for early in before_request {
        let early = rewritten(&denied, 2, &|event| event["ts"] = early.into());
        assert_eq!(replay(&early), Err(ReplayError::OutOfOrder));
    }
    let same = rewritten(&denied, 2, &|event| event["ts"] = T0.into());
    assert!(replay(&same).is_ok());
}

#[test]
fn events_outside_the_validity_of_the_approval_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    let (_, log) = approved_run("act-01", "appr-01", &verdict);
    let at_instant = |index: usize, ts: &'static str| {
        let log = rewritten(&log, index, &|event| event["ts"] = ts.into());
        replay(&log[..=index])
    };
    // Asked at the instant it expires, granted or started at that instant or after.
    assert_eq!(at_instant(1, EXPIRES), Err(ReplayError::OutsideValidity));
    for late in [EXPIRES, AFTER] {
        assert_eq!(at_instant(2, late), Err(ReplayError::OutsideValidity));
        assert_eq!(at_instant(3, late), Err(ReplayError::OutsideValidity));
    }
    assert!(at_instant(3, "2030-01-02T03:19:04.999999Z").is_ok());

    // Expired before its validity was over.
    let (action, planned, _) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    let (action, requested) = action.apply(ask("appr-01"), meta(T0, 1)).unwrap();
    let expire = Command::Expire {
        action_id: id("act-01"),
        approval_id: id("appr-01"),
        plan_hash: action.plan_hash().clone(),
    };
    let (action, expired) = action.apply(expire, meta(EXPIRES, 2)).unwrap();
    let log = [planned, requested, expired];
    assert_eq!(replay(&log), Ok(action.snapshot()));
    let early = rewritten(&log, 2, &|event| event["ts"] = T1.into());
    assert_eq!(replay(&early), Err(ReplayError::OutsideValidity));
}

#[test]
fn valid_events_this_gate_never_writes_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    let (_, log) = approved_run("act-01", "appr-01", &verdict);
    let not_ours = |log: Vec<Event>| matches!(replay(&log), Err(ReplayError::NotFromThisGate(_)));

    assert!(not_ours(rewritten(&log[..1], 0, &|event| {
        event["data"]["risk"] = "high".into();
    })));
    assert!(not_ours(rewritten(&log[..1], 0, &|event| {
        event["data"]["needs_elevation"] = true.into();
    })));
    // An approval asked for a plan of low risk, or with another risk than the plan.
    assert!(not_ours(rewritten(&log[..2], 0, &|event| {
        event["data"]["risk"] = "low".into();
    })));
    assert!(not_ours(rewritten(&log[..2], 1, &|event| {
        event["data"]["risk"] = "high".into();
    })));
    assert!(not_ours(rewritten(&log[..2], 1, &|event| {
        event["data"]["needs_elevation"] = true.into();
    })));

    // What does not even read as an event of the gate: version 1 of the request, which names
    // no action, an event of another family and a stream of another entity.
    let mut old = log[1].to_json();
    old["type"] = "urn:ads:event:approval.requested:1".into();
    old["data"].as_object_mut().unwrap().remove("action_id");
    assert!(Event::from_json(old).is_err());
    let mut foreign = log[0].to_json();
    foreign["type"] = "urn:ads:event:task.created:1".into();
    assert!(Event::from_json(foreign).is_err());
    let mut moved = log[2].to_json();
    moved["stream"] = "approval/appr-02".into();
    assert!(Event::from_json(moved).is_err());
}

#[test]
fn denied_action_is_rebuilt_as_final() {
    let verdict = denied();
    let (action, event, _) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    let snapshot = replay(std::slice::from_ref(&event)).unwrap();
    assert_eq!(snapshot, action.snapshot());
    assert_eq!(snapshot.state, State::Denied);
    assert_eq!(snapshot.plan_hash, verdict.plan().hash());

    // No event follows `action.denied`, not even the ones of the same action and plan.
    let (_, log) = approved_run("act-01", "appr-01", &approval_needed(&[BUILD]));
    for later in &log {
        assert!(unexpected(replay(&[event.clone(), later.clone()])));
    }
}
