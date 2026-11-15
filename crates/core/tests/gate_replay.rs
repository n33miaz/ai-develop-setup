//! The state of an action rebuilt from its events alone: on every path of the state machine,
//! at every step, the log gives the state the machine holds in memory. And the logs the
//! machine would never write are refused.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use ads_core::gate::{
    Action, ActionPlan, Command, Decision, ErrorCode, Event, ExecutionPermit,
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

/// Walks every path of the machine from one planned action, trying every command at every
/// state. At each step the log so far must rebuild the state in memory.
struct Walk {
    plan: ActionPlan,
    paths: Vec<Vec<String>>,
    final_states: BTreeSet<String>,
    events: Vec<Value>,
}

impl Walk {
    fn step(
        &mut self,
        action: &Action,
        permit: Option<&ExecutionPermit>,
        log: &[Event],
        seqs: &BTreeMap<String, u64>,
    ) {
        // Straight from memory, and from the log as it would be read back from storage.
        assert_eq!(replay(log), Ok(action.snapshot()), "{log:?}");
        let read_back: Vec<Event> = log
            .iter()
            .map(|event| Event::from_json(event.to_json()).unwrap())
            .collect();
        assert_eq!(read_back, log);
        assert_eq!(replay(&read_back), Ok(action.snapshot()));

        // `seq` counts in the stream of the event to come.
        let seq_of = |family: &str| {
            let stream = match family {
                "action" => format!("action/{}", action.action_id()),
                _ => "approval/appr-01".to_owned(),
            };
            seqs.get(&stream).copied().unwrap_or(0) + 1
        };
        let hash = action.plan_hash().clone();
        let mut moves: Vec<(Action, Option<ExecutionPermit>, Event)> = Vec::new();
        if let Ok((next, permit, event)) = action.authorize(&self.plan, meta(T1, seq_of("action")))
        {
            moves.push((next, Some(permit), event));
        }
        let mut commands = vec![
            (
                "approval",
                T0,
                Command::RequestApproval {
                    approval_id: id("appr-01"),
                    summary: ShortText::new("Apagar a pasta de build").unwrap(),
                    expires_at: at(EXPIRES),
                },
            ),
            (
                "approval",
                T1,
                Command::Grant {
                    approval_id: id("appr-01"),
                    plan_hash: hash.clone(),
                    via: Via::LocalUi,
                },
            ),
            (
                "approval",
                T1,
                Command::Deny {
                    approval_id: id("appr-01"),
                    plan_hash: hash.clone(),
                    reason: None,
                },
            ),
            (
                "approval",
                AFTER,
                Command::Expire {
                    approval_id: id("appr-01"),
                    plan_hash: hash,
                },
            ),
            (
                "action",
                T1,
                Command::Fail {
                    error_code: ErrorCode::new("E_PLAN_CHANGED").unwrap(),
                },
            ),
        ];
        if let Some(permit) = permit {
            let permit = permit.clone();
            commands.push(("action", T1, Command::Executed { permit }));
        }
        for (family, ts, command) in commands {
            if let Ok((next, event)) = action.apply(command, meta(ts, seq_of(family))) {
                moves.push((next, permit.cloned(), event));
            }
        }

        if moves.is_empty() {
            assert!(
                action.state().is_terminal(),
                "stuck in {:?}",
                action.state()
            );
            self.final_states.insert(format!("{:?}", action.state()));
            self.paths.push(
                log.iter()
                    .map(|event| event.kind().rsplit(':').nth(1).unwrap().to_owned())
                    .collect(),
            );
            return;
        }
        assert!(
            !action.state().is_terminal(),
            "{:?} moved on",
            action.state()
        );
        for (next, permit, event) in moves {
            let json = event.to_json();
            stream_names_the_entity_in_data(&json);
            let mut seqs = seqs.clone();
            let seq = seqs.entry(event.stream().to_owned()).or_insert(0);
            *seq += 1;
            assert_eq!(json["seq"], *seq, "{json}");
            self.events.push(json);
            let mut log = log.to_vec();
            log.push(event);
            self.step(&next, permit.as_ref(), &log, &seqs);
        }
    }
}

fn walk(verdict: &Verdict) -> Walk {
    let (action, event, _) = Action::plan(id("act-01"), verdict, meta(T0, 1)).unwrap();
    let json = event.to_json();
    stream_names_the_entity_in_data(&json);
    let mut walk = Walk {
        plan: verdict.plan().clone(),
        paths: Vec::new(),
        final_states: BTreeSet::new(),
        events: vec![json],
    };
    let seqs = BTreeMap::from([(event.stream().to_owned(), 1)]);
    walk.step(&action, None, &[event], &seqs);
    walk.paths.sort();
    walk
}

fn paths(walk: &Walk) -> Vec<String> {
    walk.paths.iter().map(|path| path.join(" > ")).collect()
}

#[test]
fn replay_gives_the_state_in_memory_on_every_path_of_the_machine() {
    let allowed = walk(&allowed());
    assert_eq!(
        paths(&allowed),
        [
            "action.planned > action.failed",
            "action.planned > action.started > action.executed",
            "action.planned > action.started > action.failed",
        ]
    );
    let approval = walk(&approval_needed(&[BUILD]));
    assert_eq!(
        paths(&approval),
        [
            "action.planned > action.failed",
            "action.planned > approval.requested > approval.denied",
            "action.planned > approval.requested > approval.expired",
            "action.planned > approval.requested > approval.granted > action.failed",
            "action.planned > approval.requested > approval.granted > action.started > action.executed",
            "action.planned > approval.requested > approval.granted > action.started > action.failed",
        ]
    );
    let denied = walk(&denied());
    assert_eq!(paths(&denied), ["action.denied"]);

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
        .flat_map(|walk| {
            walk.events
                .iter()
                .map(|event| event["type"].as_str().unwrap())
        })
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from([
            "urn:ads:event:action.denied:1",
            "urn:ads:event:action.executed:1",
            "urn:ads:event:action.failed:1",
            "urn:ads:event:action.planned:1",
            "urn:ads:event:action.started:1",
            "urn:ads:event:approval.denied:1",
            "urn:ads:event:approval.expired:1",
            "urn:ads:event:approval.granted:1",
            "urn:ads:event:approval.requested:2",
        ])
    );
}

#[test]
fn every_emitted_event_is_in_the_stream_of_the_entity_in_its_data() {
    let mut checked = 0;
    for verdict in [allowed(), approval_needed(&[BUILD]), denied()] {
        for event in walk(&verdict).events {
            stream_names_the_entity_in_data(&event);
            checked += 1;
        }
    }
    assert!(checked >= 16, "only {checked} events checked");

    // The approval events sit in the stream of the approval, and the request ties it to the
    // action: nothing else says which action an approval belongs to.
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
    assert_eq!(log[3].to_json()["data"]["approval_id"], "appr-01");
}

/// The longest path: planned, approval requested and granted, started, executed.
fn approved_run(action_id: &str, approval_id: &str, verdict: &Verdict) -> (Action, Vec<Event>) {
    let (action, planned, _) = Action::plan(id(action_id), verdict, meta(T0, 1)).unwrap();
    let request = Command::RequestApproval {
        approval_id: id(approval_id),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at: at(EXPIRES),
    };
    let (action, requested) = action.apply(request, meta(T0, 1)).unwrap();
    let grant = Command::Grant {
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

fn unexpected(result: Result<ads_core::gate::ActionSnapshot, ReplayError>) -> bool {
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
    // Anything after a final state, the same event twice included.
    for again in &log {
        let mut longer = log.clone();
        longer.push(again.clone());
        assert!(unexpected(replay(&longer)), "{}", again.kind());
    }
    for twice in 0..log.len() {
        let mut repeated = log[..=twice].to_vec();
        repeated.push(log[twice].clone());
        assert!(replay(&repeated).is_err(), "event {twice} twice");
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
    assert_eq!(splice(1, &other_action), Err(ReplayError::ActionMismatch));
    assert_eq!(splice(3, &other_action), Err(ReplayError::ActionMismatch));
    assert_eq!(splice(4, &other_action), Err(ReplayError::ActionMismatch));

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
    let request = Command::RequestApproval {
        approval_id: id("appr-01"),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at: at(EXPIRES),
    };
    let (action, requested) = action.apply(request, meta(T0, 1)).unwrap();
    let expire = Command::Expire {
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
