//! The contract types of the gate against the golden files of the contracts, every version of
//! them. A valid golden file that stops matching the types, an invalid one they start to
//! accept, or a generated event or record that drifts from the golden shape fails here. Reads
//! the golden files; writes nothing.

mod common;

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::Path;

use ads_core::gate::{
    Action, ActionDenied, ActionExecuted, ActionFailed, ActionPlan, ActionPlanRecord,
    ActionPlanned, ActionStarted, ApprovalDenied, ApprovalExpired, ApprovalGranted,
    ApprovalRequested, ApprovalRequestedV1, Command, Decision, Envelope, ErrorCode, Estimate,
    Event, EventMeta, Id, OperationRequest as Op, PathText, PlanRequest, RuleCode, ShortText,
    TargetRequest, Timestamp, Verdict, Via, classify,
};
use common::{POSIX_HOME, WIN_HOME, WIN_ROOT, at, canonical_of, policy, request, sha256_of, win};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Invalid golden files the types are known not to refuse, as `folder/file`. Empty on purpose.
const NOT_COVERED_BY_TYPES: &[&str] = &[];

/// The golden folders of the gate: its events, its record and the two common types that came
/// with them.
const GATE_FOLDERS: &[&str] = &[
    "common.abspath.v1",
    "common.action-rule.v1",
    "event.action.denied.v1",
    "event.action.executed.v1",
    "event.action.failed.v1",
    "event.action.planned.v1",
    "event.action.started.v1",
    "event.approval.denied.v1",
    "event.approval.expired.v1",
    "event.approval.granted.v1",
    "event.approval.requested.v1",
    "event.approval.requested.v2",
    "record.action-plan.v1",
];

const BUILD: &str = "C:\\Users\\user-a\\work\\proj\\build";
/// Before the instant of every golden event.
const EARLIER: &str = "2030-01-02T03:00:00Z";
/// After the instant of every golden event.
const LATER: &str = "2030-01-02T05:00:00Z";

fn golden_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden")
}

/// The files of `golden/<folder>/<kind>`; `folder` carries the version (`event.x.v2`).
fn golden_files(folder: &str, kind: &str) -> Vec<(String, Value)> {
    let dir = golden_root().join(folder).join(kind);
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

fn types_match<T: Serialize + DeserializeOwned>(folder: &str) {
    let valid = golden_files(folder, "valid");
    assert!(
        valid.len() >= 2,
        "{folder}: expected at least 2 valid files"
    );
    for (name, golden) in valid {
        let typed: T = serde_json::from_value(golden.clone())
            .unwrap_or_else(|err| panic!("{folder}/valid/{name} no longer matches: {err}"));
        // Compared as JSON values: key order is not part of the contract.
        assert_eq!(
            serde_json::to_value(&typed).unwrap(),
            golden,
            "{folder}/valid/{name} changed on the way back"
        );
    }
    let invalid = golden_files(folder, "invalid");
    assert!(
        invalid.len() >= 3,
        "{folder}: expected at least 3 invalid files"
    );
    for (name, golden) in invalid {
        let accepted = serde_json::from_value::<T>(golden).is_ok();
        let known_gap = NOT_COVERED_BY_TYPES.contains(&format!("{folder}/{name}").as_str());
        assert_eq!(
            accepted, known_gap,
            "{folder}/invalid/{name}: accepted = {accepted}, listed as a known gap = {known_gap}"
        );
    }
}

#[test]
fn types_match_the_golden_files_of_every_version() {
    let mut checked = BTreeSet::new();
    let mut check = |folder: &'static str, matches: fn(&str)| {
        matches(folder);
        checked.insert(folder);
    };
    check("common.abspath.v1", types_match::<PathText>);
    check("common.action-rule.v1", types_match::<RuleCode>);
    check(
        "event.action.planned.v1",
        types_match::<Envelope<ActionPlanned>>,
    );
    check(
        "event.action.denied.v1",
        types_match::<Envelope<ActionDenied>>,
    );
    check(
        "event.action.started.v1",
        types_match::<Envelope<ActionStarted>>,
    );
    check(
        "event.action.executed.v1",
        types_match::<Envelope<ActionExecuted>>,
    );
    check(
        "event.action.failed.v1",
        types_match::<Envelope<ActionFailed>>,
    );
    check(
        "event.approval.requested.v1",
        types_match::<Envelope<ApprovalRequestedV1>>,
    );
    check(
        "event.approval.requested.v2",
        types_match::<Envelope<ApprovalRequested>>,
    );
    check(
        "event.approval.granted.v1",
        types_match::<Envelope<ApprovalGranted>>,
    );
    check(
        "event.approval.denied.v1",
        types_match::<Envelope<ApprovalDenied>>,
    );
    check(
        "event.approval.expired.v1",
        types_match::<Envelope<ApprovalExpired>>,
    );
    check("record.action-plan.v1", types_match::<ActionPlanRecord>);
    assert_eq!(checked, GATE_FOLDERS.iter().copied().collect());
}

/// A new event, record or version of the gate in `contracts/golden` fails here until a type
/// covers it.
#[test]
fn every_golden_folder_of_the_gate_is_covered() {
    let of_the_gate = |name: &str| {
        ["event.action.", "event.approval.", "record.action-plan."]
            .iter()
            .any(|prefix| name.starts_with(prefix))
            || GATE_FOLDERS.contains(&name)
    };
    let found: BTreeSet<String> = std::fs::read_dir(golden_root())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| of_the_gate(name))
        .collect();
    let covered: BTreeSet<String> = GATE_FOLDERS.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(found, covered);
}

#[test]
fn envelope_rules_shared_by_every_event_are_enforced() {
    let (_, base) = golden_files("event.action.planned.v1", "valid").remove(0);
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
    // The stream of another entity than the one in `data`.
    assert!(broken(&|event| event["stream"] = "action/act-02".into()));
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

fn denied() -> Verdict {
    let verdict = win(Op::DeleteRecursive, &[WIN_ROOT]);
    assert_eq!(verdict.decision(), Decision::Denied);
    verdict
}

fn planned(action_id: Id, verdict: &Verdict) -> Action {
    Action::plan(action_id, verdict, earlier()).unwrap().0
}

/// An action of `action_id` waiting for the approval `approval_id`, valid until `expires_at`.
fn waiting(action_id: Id, approval_id: Id, expires_at: Timestamp) -> Action {
    let command = Command::RequestApproval {
        approval_id,
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at,
    };
    planned(action_id, &approval_needed())
        .apply(command, earlier())
        .unwrap()
        .0
}

/// An action whose approval `approval_id` was granted, or that needs none.
fn ready(action_id: Id, approval_id: Option<Id>) -> (Action, ActionPlan) {
    let Some(approval_id) = approval_id else {
        let verdict = allowed();
        return (planned(action_id, &verdict), verdict.plan().clone());
    };
    let action = waiting(action_id, approval_id.clone(), at(LATER));
    let grant = Command::Grant {
        approval_id,
        plan_hash: action.plan_hash().clone(),
        via: Via::LocalUi,
    };
    let (action, _) = action.apply(grant, earlier()).unwrap();
    (action, approval_needed().plan().clone())
}

fn act(text: &str) -> Id {
    Id::new(text).unwrap()
}

/// The generated event must be the golden one, stream included, except for the fields of
/// `data` listed in `derived`: those come from the plan, and the golden files carry sample
/// values for them.
fn same_as_golden(folder: &str, name: &str, event: &Event, golden: &Value, derived: &[&str]) {
    let generated = event.to_json();
    let mut expected = golden.clone();
    for field in derived {
        expected["data"][*field] = generated["data"][*field].clone();
    }
    assert_eq!(generated, expected, "{folder}/valid/{name}");
}

#[test]
fn generated_action_planned_is_the_golden_event() {
    let folder = "event.action.planned.v1";
    let mut compared = 0;
    for (name, golden) in golden_files(folder, "valid") {
        // The gate never plans an elevated action, and a `high` plan is a denied one.
        let verdict = match (
            golden["data"]["risk"].as_str().unwrap(),
            golden["data"]["needs_elevation"].as_bool().unwrap(),
        ) {
            ("low", false) => allowed(),
            ("medium", false) => approval_needed(),
            _ => continue,
        };
        let (action, event, record) =
            Action::plan(data_id(&golden, "action_id"), &verdict, meta_of(&golden)).unwrap();
        same_as_golden(folder, &name, &event, &golden, &["verb", "plan_hash"]);
        let hash = &event.to_json()["data"]["plan_hash"];
        assert_eq!(hash, action.plan_hash().as_str());
        assert_eq!(hash, &record.to_json()["plan_hash"]);
        compared += 1;
    }
    assert!(compared >= 1, "no golden action.planned could be generated");
}

#[test]
fn generated_action_denied_is_the_golden_event() {
    let folder = "event.action.denied.v1";
    for (name, golden) in golden_files(folder, "valid") {
        let verdict = denied();
        let (_, event, record) =
            Action::plan(data_id(&golden, "action_id"), &verdict, meta_of(&golden)).unwrap();
        same_as_golden(
            folder,
            &name,
            &event,
            &golden,
            &["verb", "plan_hash", "rules"],
        );
        let generated = event.to_json();
        assert_eq!(generated["data"]["verb"], "fs.delete_recursive");
        assert_eq!(
            generated["data"]["rules"],
            serde_json::json!([
                "work_root_itself",
                "outside_allowed_roots",
                "quarantine_itself"
            ])
        );
        assert_eq!(
            generated["data"]["plan_hash"],
            record.to_json()["plan_hash"]
        );
    }
}

#[test]
fn generated_approval_requested_is_the_golden_event_of_version_2() {
    let folder = "event.approval.requested.v2";
    let mut compared = 0;
    for (name, golden) in golden_files(folder, "valid") {
        if golden["data"]["risk"] != "medium" || golden["data"]["needs_elevation"] != false {
            continue;
        }
        let action = planned(data_id(&golden, "action_id"), &approval_needed());
        let command = Command::RequestApproval {
            approval_id: data_id(&golden, "approval_id"),
            summary: serde_json::from_value(golden["data"]["summary"].clone()).unwrap(),
            expires_at: serde_json::from_value(golden["data"]["expires_at"].clone()).unwrap(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
        compared += 1;
    }
    assert!(
        compared >= 1,
        "no golden approval.requested could be generated"
    );
}

#[test]
fn generated_approval_decisions_are_the_golden_events() {
    let folder = "event.approval.granted.v1";
    for (name, golden) in golden_files(folder, "valid") {
        let action = waiting(act("act-01"), data_id(&golden, "approval_id"), at(LATER));
        let command = Command::Grant {
            approval_id: data_id(&golden, "approval_id"),
            plan_hash: action.plan_hash().clone(),
            via: serde_json::from_value(golden["data"]["via"].clone()).unwrap(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
    }
    let folder = "event.approval.denied.v1";
    for (name, golden) in golden_files(folder, "valid") {
        let action = waiting(act("act-01"), data_id(&golden, "approval_id"), at(LATER));
        let command = Command::Deny {
            approval_id: data_id(&golden, "approval_id"),
            plan_hash: action.plan_hash().clone(),
            reason: serde_json::from_value(golden["data"]["reason"].clone()).unwrap(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
    }
    let folder = "event.approval.expired.v1";
    for (name, golden) in golden_files(folder, "valid") {
        // Valid exactly until the instant of the golden event.
        let expires_at: Timestamp = serde_json::from_value(golden["ts"].clone()).unwrap();
        let action = waiting(act("act-01"), data_id(&golden, "approval_id"), expires_at);
        let command = Command::Expire {
            approval_id: data_id(&golden, "approval_id"),
            plan_hash: action.plan_hash().clone(),
        };
        let (_, event) = action.apply(command, meta_of(&golden)).unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
    }
}

#[test]
fn generated_action_started_is_the_golden_event() {
    let folder = "event.action.started.v1";
    let mut with_approval = 0;
    for (name, golden) in golden_files(folder, "valid") {
        let approval: Option<Id> =
            serde_json::from_value(golden["data"]["approval_id"].clone()).unwrap();
        with_approval += usize::from(approval.is_some());
        let (action, plan) = ready(data_id(&golden, "action_id"), approval);
        let (_, permit, event) = action.authorize(&plan, meta_of(&golden)).unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
        assert_eq!(
            event.to_json()["data"]["plan_hash"],
            permit.plan_hash().as_str()
        );
    }
    assert!(with_approval >= 1, "no golden action.started with approval");
}

#[test]
fn generated_action_results_are_the_golden_events() {
    let folder = "event.action.executed.v1";
    for (name, golden) in golden_files(folder, "valid") {
        let approval: Option<Id> =
            serde_json::from_value(golden["data"]["approval_id"].clone()).unwrap();
        let (action, plan) = ready(data_id(&golden, "action_id"), approval);
        let (action, permit, _) = action.authorize(&plan, earlier()).unwrap();
        let (_, event) = action
            .apply(Command::Executed { permit }, meta_of(&golden))
            .unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
    }
    let folder = "event.action.failed.v1";
    for (name, golden) in golden_files(folder, "valid") {
        let action = planned(data_id(&golden, "action_id"), &allowed());
        let error_code: ErrorCode =
            serde_json::from_value(golden["data"]["error_code"].clone()).unwrap();
        let (_, event) = action
            .apply(Command::Fail { error_code }, meta_of(&golden))
            .unwrap();
        same_as_golden(folder, &name, &event, &golden, &["plan_hash"]);
    }
}

// ---- record.action-plan ----------------------------------------------------------------------

const RECORD: &str = "record.action-plan.v1";

fn golden_record(name: &str) -> Value {
    golden_files(RECORD, "valid")
        .into_iter()
        .find(|(file, _)| file == name)
        .unwrap_or_else(|| panic!("{RECORD}/valid/{name} is gone"))
        .1
}

/// The request a caller would send for a golden record: the targets as loose text, last first
/// and misspelled, so the order and the spelling of the record come from the core.
fn request_of(golden: &Value) -> PlanRequest {
    let text = |value: &Value| value.as_str().unwrap().to_owned();
    let operation = match golden["operation"].as_str().unwrap() {
        "fs.delete" => Op::Delete,
        "fs.delete_recursive" => Op::DeleteRecursive,
        "fs.overwrite" => Op::Overwrite,
        "fs.clean_untracked" => Op::CleanUntracked,
        "fs.move" => Op::Move {
            destination: text(&golden["destination"]),
        },
        other => panic!("unknown operation {other}"),
    };
    let targets = golden["targets"].as_array().unwrap().iter().rev();
    let targets = targets.map(|target| {
        let path = text(&target["path"]);
        let separator = if path.starts_with('/') { "/" } else { "\\" };
        // Not the first separator: two of them at the start would be a network path.
        let (head, tail) = path.split_at(path.find(separator).unwrap() + 1);
        let doubled = format!(
            "{head}{}",
            tail.replacen(separator, &separator.repeat(2), 1)
        );
        let path = match target["pattern"].as_str() {
            Some(pattern) => format!("{doubled}{separator}{pattern}"),
            None => format!("{doubled}{separator}"),
        };
        TargetRequest {
            path,
            repo_root: target["repo_root"].as_bool().unwrap(),
        }
    });
    PlanRequest {
        operation,
        targets: targets.collect(),
        work_root: text(&golden["work_root"]),
        estimate: golden["estimate"].as_object().map(|estimate| Estimate {
            items: estimate["items"].as_u64().unwrap(),
            bytes: estimate["bytes"].as_u64().unwrap(),
        }),
    }
}

/// Plans the request of a golden record in the core, under a policy that differs from the
/// shipped one only in `quarantine_enabled`, and gives back the record and the event.
fn rebuilt(golden: &Value, home: &str, quarantine: bool) -> (Value, Value) {
    let plan = ActionPlan::normalize(&request_of(golden)).unwrap();
    let overrides = format!("quarantine_enabled = {quarantine}");
    let verdict = classify(&plan, &policy(home, &overrides));
    let action_id: Id = serde_json::from_value(golden["action_id"].clone()).unwrap();
    let (_, event, record) = Action::plan(action_id, &verdict, earlier()).unwrap();
    (record.to_json(), event.to_json())
}

/// The vector of `contracts/README.md`: the first valid golden record, rebuilt by the core
/// from a plan, with the same record and the same hash.
#[test]
fn first_golden_action_plan_is_rebuilt_by_the_core() {
    let golden = golden_record("01-base.json");
    let hash = "sha256:c7b870d89ef435eb431fcdf8e99f82f2b10c9b05a25eb86eb2ed603fa366eb94";
    assert_eq!(golden["plan_hash"], hash);

    let plan = ActionPlan::normalize(&PlanRequest {
        operation: Op::DeleteRecursive,
        targets: ["/work/proj/b", "/work/proj//a/"]
            .map(|path| TargetRequest {
                path: path.to_owned(),
                repo_root: false,
            })
            .to_vec(),
        work_root: "/work/proj".to_owned(),
        estimate: Some(Estimate {
            items: 3,
            bytes: 4096,
        }),
    })
    .unwrap();
    assert_eq!(
        plan.canonical(),
        "ads.action-plan/1\n\
         op:fs.delete_recursive\n\
         work_root:10:/work/proj\n\
         destination:-\n\
         estimate:3:4096\n\
         targets:2\n\
         target:0:12:/work/proj/a\n\
         target:0:12:/work/proj/b\n"
    );
    assert_eq!(plan.hash().as_str(), hash);

    let verdict = classify(&plan, &policy(POSIX_HOME, "quarantine_enabled = false"));
    let (action, event, record) = Action::plan(act("act-01"), &verdict, earlier()).unwrap();
    assert_eq!(record.to_json(), golden);
    assert_eq!(record.canonical(), plan.canonical());
    assert_eq!(action.plan_hash().as_str(), hash);
    assert_eq!(event.to_json()["data"]["plan_hash"], hash);
}

#[test]
fn every_golden_action_plan_is_rebuilt_by_the_core() {
    // The policy of each golden record: whose home, and whether quarantine is on.
    let cases = [
        ("01-base.json", POSIX_HOME, false, "action.planned"),
        ("02-quarantined.json", WIN_HOME, true, "action.planned"),
        (
            "03-requires-approval.json",
            WIN_HOME,
            true,
            "action.planned",
        ),
        ("04-denied.json", POSIX_HOME, false, "action.denied"),
        ("05-move.json", POSIX_HOME, true, "action.planned"),
    ];
    assert_eq!(cases.len(), golden_files(RECORD, "valid").len());
    for (name, home, quarantine, kind) in cases {
        let golden = golden_record(name);
        let (record, event) = rebuilt(&golden, home, quarantine);
        assert_eq!(record, golden, "{name}");
        assert_eq!(event["data"]["plan_hash"], golden["plan_hash"], "{name}");
        assert_eq!(event["type"], format!("urn:ads:event:{kind}:1"), "{name}");
    }
}

#[test]
fn plan_hash_of_every_golden_record_is_the_canonical_serialization() {
    for (name, golden) in golden_files(RECORD, "valid") {
        let record: ActionPlanRecord = serde_json::from_value(golden.clone()).unwrap();
        // The text and the digest are computed here, apart from the core.
        assert_eq!(record.canonical(), canonical_of(&golden), "{name}");
        assert_eq!(
            golden["plan_hash"],
            sha256_of(&canonical_of(&golden)),
            "{name}"
        );
    }
}

#[test]
fn record_rules_the_schema_cannot_express_are_enforced() {
    let refused = |name: &str, change: &dyn Fn(&mut Value)| {
        let mut record = golden_record(name);
        change(&mut record);
        serde_json::from_value::<ActionPlanRecord>(record).is_err()
    };
    assert!(!refused("01-base.json", &|_| {}));
    // Any change to what the hash covers, with the old hash.
    assert!(refused("01-base.json", &|record| {
        record["estimate"]["items"] = 4.into();
    }));
    assert!(refused("01-base.json", &|record| {
        record["targets"][0]["repo_root"] = true.into();
    }));
    assert!(refused("01-base.json", &|record| {
        record["work_root"] = "/work".into();
    }));
    assert!(refused("02-quarantined.json", &|record| {
        record["destination"] = "C:\\work\\proj\\.ads\\quarantine\\0000000000000000".into();
    }));
    // Targets out of the canonical order, or repeated, even with the hash of that text.
    let rehashed = |record: &mut Value| {
        record["plan_hash"] = sha256_of(&canonical_of(record)).into();
    };
    assert!(refused("01-base.json", &|record| {
        record["targets"].as_array_mut().unwrap().reverse();
        rehashed(record);
    }));
    assert!(refused("01-base.json", &|record| {
        let first = record["targets"][0].clone();
        record["targets"][1] = first;
        rehashed(record);
    }));
    // The same change with the right order is accepted: the check above is about the order.
    assert!(!refused("01-base.json", &|record| {
        record["targets"][1]["path"] = "/work/proj/c".into();
        rehashed(record);
    }));
    assert!(refused("01-base.json", &|record| {
        record["version"] = 2.into();
    }));
}
