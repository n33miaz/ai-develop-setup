//! The events of the action state machine against the golden files of the contracts. A valid
//! golden file that stops matching the types, an invalid one they start to accept, or a
//! generated event that drifts from the golden shape fails here. Reads the golden files; writes
//! nothing.

mod common;

use std::num::NonZeroU64;
use std::path::Path;

use ads_core::gate::{
    Action, ActionExecuted, ActionFailed, ActionPlan, ActionPlanned, ApprovalDenied,
    ApprovalExpired, ApprovalGranted, ApprovalRequested, Command, Decision, Envelope, ErrorCode,
    Event, EventData, EventMeta, Id, OperationRequest as Op, PlanHash, ShortText, Timestamp,
    Verdict, classify,
};
use common::{WIN_HOME, WIN_ROOT, at, policy, request, win};
use serde_json::Value;

/// Invalid golden files the types are known not to refuse, as `schema/file`. Empty on purpose.
const NOT_COVERED_BY_TYPES: &[&str] = &[];

/// Valid golden files whose stream names another entity than their `data` does. Empty on
/// purpose: the contracts tie the two, and the machine always writes the stream of the entity
/// in `data`.
const STREAM_OF_ANOTHER_ENTITY: &[&str] = &[];

const BUILD: &str = "C:\\Users\\user-a\\work\\proj\\build";
/// Before the instant of every golden event.
const EARLIER: &str = "2030-01-02T03:00:00Z";
/// After the instant of every golden event.
const LATER: &str = "2030-01-02T03:19:05Z";

fn golden_files(schema: &str, kind: &str) -> Vec<(String, Value)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/golden")
        .join(format!("{schema}.v1"))
        .join(kind);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        // The index says why each invalid file is invalid; it is not an example.
        .filter(|path| path.file_name().is_some_and(|name| name != "index.json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            (name, serde_json::from_str(&text).unwrap())
        })
        .collect()
}

fn types_match<D: EventData>(schema: &str) {
    let valid = golden_files(schema, "valid");
    assert!(
        valid.len() >= 2,
        "{schema}: expected at least 2 valid files"
    );
    for (name, golden) in valid {
        let typed: Envelope<D> = serde_json::from_value(golden.clone())
            .unwrap_or_else(|err| panic!("{schema}/valid/{name} no longer matches: {err}"));
        // Compared as JSON values: key order is not part of the contract.
        assert_eq!(
            serde_json::to_value(&typed).unwrap(),
            golden,
            "{schema}/valid/{name} changed on the way back"
        );
    }
    let invalid = golden_files(schema, "invalid");
    assert!(
        invalid.len() >= 3,
        "{schema}: expected at least 3 invalid files"
    );
    for (name, golden) in invalid {
        let accepted = serde_json::from_value::<Envelope<D>>(golden).is_ok();
        let known_gap = NOT_COVERED_BY_TYPES.contains(&format!("{schema}/{name}").as_str());
        assert_eq!(
            accepted, known_gap,
            "{schema}/invalid/{name}: accepted = {accepted}, listed as a known gap = {known_gap}"
        );
    }
}

#[test]
fn event_types_match_the_golden_files() {
    types_match::<ActionPlanned>("event.action.planned");
    types_match::<ActionExecuted>("event.action.executed");
    types_match::<ActionFailed>("event.action.failed");
    types_match::<ApprovalRequested>("event.approval.requested");
    types_match::<ApprovalGranted>("event.approval.granted");
    types_match::<ApprovalDenied>("event.approval.denied");
    types_match::<ApprovalExpired>("event.approval.expired");
}

#[test]
fn envelope_rules_shared_by_every_event_are_enforced() {
    let (_, base) = golden_files("event.action.planned", "valid").remove(0);
    let broken = |change: &dyn Fn(&mut Value)| {
        let mut event = base.clone();
        change(&mut event);
        serde_json::from_value::<Envelope<ActionPlanned>>(event).is_err()
    };
    assert!(broken(&|event| event["seq"] = 0.into()));
    assert!(broken(&|event| event["seq"] = Value::from(1.5)));
    assert!(broken(&|event| event["id"] = "not-a-ulid".into()));
    assert!(broken(&|event| event["ts"] = "2030-01-02".into()));
    assert!(broken(&|event| event["data_class"] = "open".into()));
    assert!(broken(&|event| event["actor"]["kind"] = "robot".into()));
    assert!(broken(&|event| event["extra"] = true.into()));
    assert!(broken(&|event| event["stream"] = "action/".into()));
    assert!(broken(&|event| event["stream"] = "actions/act-01".into()));
    assert!(broken(&|event| {
        event.as_object_mut().unwrap().remove("cause");
    }));
    assert!(!broken(&|_| {}));
}

/// The envelope fields the caller supplies, taken from a golden event.
fn meta_of(golden: &Value) -> EventMeta {
    let field = |name: &str| golden[name].clone();
    EventMeta {
        id: serde_json::from_value(field("id")).unwrap(),
        ts: serde_json::from_value(field("ts")).unwrap(),
        seq: NonZeroU64::new(golden["seq"].as_u64().unwrap()).unwrap(),
        actor: serde_json::from_value(field("actor")).unwrap(),
        cause: serde_json::from_value(field("cause")).unwrap(),
        data_class: serde_json::from_value(field("data_class")).unwrap(),
    }
}

fn earlier() -> EventMeta {
    common::meta(EARLIER, 1)
}

fn data_id(golden: &Value, field: &str) -> Id {
    serde_json::from_value(golden["data"][field].clone()).unwrap()
}

/// A recursive delete with no estimate: requires approval, so its risk is `medium`.
fn approval_needed() -> Verdict {
    let mut request = request(Op::DeleteRecursive, WIN_ROOT, &[BUILD]);
    request.estimate = None;
    let verdict = classify(
        &ActionPlan::normalize(&request).unwrap(),
        &policy(WIN_HOME, ""),
    );
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
    verdict
}

fn allowed() -> Verdict {
    let verdict = win(Op::Delete, &[BUILD]);
    assert_eq!(verdict.decision(), Decision::Allowed);
    verdict
}

/// An action waiting for the approval named in the golden event, valid until `expires_at`.
fn waiting(golden: &Value, expires_at: Timestamp) -> Action {
    let (action, _) =
        Action::plan(Id::new("act-01").unwrap(), &approval_needed(), earlier()).unwrap();
    let command = Command::RequestApproval {
        approval_id: data_id(golden, "approval_id"),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at,
    };
    action.apply(command, earlier()).unwrap().0
}

/// The generated event must be the golden one, except for the fields listed in `derived`:
/// those come from the plan, and the golden files carry sample values for them.
fn same_as_golden(schema: &str, name: &str, event: &Event, golden: &Value, derived: &[&str]) {
    let generated = event.to_json();
    let mut expected = golden.clone();
    for field in derived {
        expected["data"][*field] = generated["data"][*field].clone();
    }
    let known_gap = STREAM_OF_ANOTHER_ENTITY.contains(&format!("{schema}/{name}").as_str());
    assert_eq!(
        generated["stream"] != expected["stream"],
        known_gap,
        "{schema}/valid/{name}: stream {} against {}",
        generated["stream"],
        expected["stream"]
    );
    expected["stream"] = generated["stream"].clone();
    assert_eq!(generated, expected, "{schema}/valid/{name}");
}

#[test]
fn generated_action_planned_is_the_golden_event() {
    let schema = "event.action.planned";
    let mut compared = 0;
    for (name, golden) in golden_files(schema, "valid") {
        // The gate never plans an elevated action, and a `high` plan is a denied one.
        let verdict = match (
            golden["data"]["risk"].as_str().unwrap(),
            golden["data"]["needs_elevation"].as_bool().unwrap(),
        ) {
            ("low", false) => allowed(),
            ("medium", false) => approval_needed(),
            _ => continue,
        };
        let (action, event) =
            Action::plan(data_id(&golden, "action_id"), &verdict, meta_of(&golden)).unwrap();
        same_as_golden(schema, &name, &event, &golden, &["verb", "plan_hash"]);
        assert_eq!(
            event.to_json()["data"]["plan_hash"],
            action.plan_hash().as_str()
        );
        compared += 1;
    }
    assert!(compared >= 1, "no golden action.planned could be generated");
}

#[test]
fn generated_approval_requested_is_the_golden_event() {
    let schema = "event.approval.requested";
    let mut compared = 0;
    for (name, golden) in golden_files(schema, "valid") {
        if golden["data"]["risk"] != "medium" || golden["data"]["needs_elevation"] != false {
            continue;
        }
        let (action, _) =
            Action::plan(Id::new("act-01").unwrap(), &approval_needed(), earlier()).unwrap();
        let command = Command::RequestApproval {
            approval_id: data_id(&golden, "approval_id"),
            summary: serde_json::from_value(golden["data"]["summary"].clone()).unwrap(),
            expires_at: serde_json::from_value(golden["data"]["expires_at"].clone()).unwrap(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(schema, &name, &event, &golden, &["plan_hash"]);
        compared += 1;
    }
    assert!(
        compared >= 1,
        "no golden approval.requested could be generated"
    );
}

#[test]
fn generated_approval_decisions_are_the_golden_events() {
    for (name, golden) in golden_files("event.approval.granted", "valid") {
        let action = waiting(&golden, at(LATER));
        let command = Command::Grant {
            approval_id: data_id(&golden, "approval_id"),
            plan_hash: action.plan_hash().clone(),
            via: serde_json::from_value(golden["data"]["via"].clone()).unwrap(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(
            "event.approval.granted",
            &name,
            &event,
            &golden,
            &["plan_hash"],
        );
    }
    for (name, golden) in golden_files("event.approval.denied", "valid") {
        let action = waiting(&golden, at(LATER));
        let command = Command::Deny {
            approval_id: data_id(&golden, "approval_id"),
            plan_hash: action.plan_hash().clone(),
            reason: serde_json::from_value(golden["data"]["reason"].clone()).unwrap(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(
            "event.approval.denied",
            &name,
            &event,
            &golden,
            &["plan_hash"],
        );
    }
    for (name, golden) in golden_files("event.approval.expired", "valid") {
        // Valid exactly until the instant of the golden event.
        let expires_at: Timestamp = serde_json::from_value(golden["ts"].clone()).unwrap();
        let action = waiting(&golden, expires_at);
        let command = Command::Expire {
            approval_id: data_id(&golden, "approval_id"),
            plan_hash: action.plan_hash().clone(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(
            "event.approval.expired",
            &name,
            &event,
            &golden,
            &["plan_hash"],
        );
    }
}

#[test]
fn generated_action_results_are_the_golden_events() {
    for (name, golden) in golden_files("event.action.executed", "valid") {
        let action_id = data_id(&golden, "action_id");
        let approval: Option<Id> =
            serde_json::from_value(golden["data"]["approval_id"].clone()).unwrap();
        let (action, plan) = match approval {
            None => {
                let verdict = allowed();
                let (action, _) = Action::plan(action_id, &verdict, earlier()).unwrap();
                (action, verdict.plan().clone())
            }
            Some(approval_id) => {
                let verdict = approval_needed();
                let (action, _) = Action::plan(action_id, &verdict, earlier()).unwrap();
                let hash: PlanHash = action.plan_hash().clone();
                let request = Command::RequestApproval {
                    approval_id: approval_id.clone(),
                    summary: ShortText::new("Apagar a pasta de build").unwrap(),
                    expires_at: at(LATER),
                };
                let (action, _) = action.apply(request, earlier()).unwrap();
                let grant = Command::Grant {
                    approval_id,
                    plan_hash: hash,
                    via: ads_core::gate::Via::LocalUi,
                };
                let (action, _) = action.apply(grant, earlier()).unwrap();
                (action, verdict.plan().clone())
            }
        };
        let meta = meta_of(&golden);
        let (action, permit) = action.authorize(&plan, &meta.ts).unwrap();
        let (_, event) = action.apply(Command::Executed { permit }, meta).unwrap();
        same_as_golden(
            "event.action.executed",
            &name,
            &event,
            &golden,
            &["plan_hash"],
        );
    }
    for (name, golden) in golden_files("event.action.failed", "valid") {
        let verdict = allowed();
        let (action, _) = Action::plan(data_id(&golden, "action_id"), &verdict, earlier()).unwrap();
        let error_code: ErrorCode =
            serde_json::from_value(golden["data"]["error_code"].clone()).unwrap();
        let (_, event) = action
            .apply(Command::Fail { error_code }, meta_of(&golden))
            .unwrap();
        same_as_golden(
            "event.action.failed",
            &name,
            &event,
            &golden,
            &["plan_hash"],
        );
    }
}
