//! Regression tests for defects found in independent reviews of the gate. Each one failed
//! before its fix. Paths are synthetic text; nothing is touched.

mod common;

use std::marker::PhantomData;

use ads_core::gate::{
    AbsPath, Action, ActionPlan, Command, Decision, Estimate, ExecutionPermit,
    OperationRequest as Op, Rule, ShortText, State, Timestamp, TransitionError, Verdict, Via,
    classify,
};
use common::{POSIX_HOME, WIN_HOME, at, id, meta, policy, request};

const T0: &str = "2030-01-02T03:04:05Z";
const T1: &str = "2030-01-02T03:06:05Z";
const EXPIRES: &str = "2030-01-02T03:19:05Z";

/// A recursive delete with no estimate, under the shipped defaults: requires approval.
fn unmeasured_delete() -> Verdict {
    let mut asked = request(Op::DeleteRecursive, "/work/proj", &["/work/proj/build"]);
    asked.estimate = None;
    let verdict = classify(
        &ActionPlan::normalize(&asked).unwrap(),
        &policy(POSIX_HOME, ""),
    );
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
    verdict
}

/// An action of `verdict` waiting for the approval `appr-01`.
fn waiting(action: &str, verdict: &Verdict) -> Action {
    let (action, _, _) = Action::plan(id(action), verdict, meta(T0, 1)).unwrap();
    let ask = Command::RequestApproval {
        approval_id: id("appr-01"),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at: at(EXPIRES),
    };
    action.apply(ask, meta(T0, 1)).unwrap().0
}

struct Probe<T>(PhantomData<T>);

trait NotClone {
    const IS_CLONE: bool = false;
}

impl<T> NotClone for Probe<T> {}

// Chosen over the trait constant only when `T` is `Clone`.
impl<T: Clone> Probe<T> {
    const IS_CLONE: bool = true;
}

/// The rewrite to quarantine, handed to the gate again, lost the approval.
#[test]
fn reclassifying_the_quarantine_rewrite_never_lowers_the_verdict() {
    let policy = policy(POSIX_HOME, "");
    let verdict = unmeasured_delete();
    assert!(verdict.is_quarantined());

    // The rewritten plan, judged again, still needs the approval, and cannot run without it.
    let again = classify(verdict.plan(), &policy);
    assert_eq!(again.decision(), Decision::RequiresApproval);
    assert!(again.has(Rule::EstimateUnknown));
    assert_eq!(again.plan().hash(), verdict.plan().hash());
    let (action, _, _) = Action::plan(id("act-01"), &again, meta(T0, 1)).unwrap();
    assert!(action.needs_approval());
    let refusal = action.authorize(again.plan(), meta(T1, 2)).unwrap_err();
    assert_eq!(refusal.error, TransitionError::ApprovalRequired);

    // The same above the limits instead of with no estimate.
    let mut asked = request(Op::DeleteRecursive, "/work/proj", &["/work/proj/build"]);
    asked.estimate = Some(Estimate {
        items: 501,
        bytes: 1,
    });
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
    let again = classify(verdict.plan(), &policy);
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
    assert_eq!(again.decision(), Decision::RequiresApproval);
    assert!(again.has(Rule::ItemLimitExceeded));

    // A move into quarantine written by hand is judged the same way.
    let by_hand = Op::Move {
        destination: "/work/proj/.ads/quarantine/entry".to_owned(),
    };
    let mut asked = request(by_hand, "/work/proj", &["/work/proj/build"]);
    asked.estimate = None;
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
}

/// An approved action handed out two permits, a permit could be copied and the
/// same `Executing` action accepted `Executed` twice. Now none of that compiles: a transition
/// takes the action by value, and neither the action, the permit nor a command with a permit
/// can be copied. Checked when this file is compiled.
#[test]
fn action_and_permit_cannot_be_copied() {
    const { assert!(!Probe::<ExecutionPermit>::IS_CLONE) };
    const { assert!(!Probe::<Action>::IS_CLONE) };
    const { assert!(!Probe::<Command>::IS_CLONE) };
    // The probe does tell a type that can be copied.
    const { assert!(Probe::<Decision>::IS_CLONE) };
}

/// The same at run time: after the permit is out, the action that remains hands out no
/// other, and after the execution is reported nothing is left to report.
#[test]
fn approved_action_hands_out_one_permit_and_closes_once() {
    let verdict = unmeasured_delete();
    let waiting = waiting("act-01", &verdict);
    let grant = Command::Grant {
        action_id: id("act-01"),
        approval_id: id("appr-01"),
        plan_hash: waiting.plan_hash().clone(),
        via: Via::LocalUi,
    };
    let (approved, _) = waiting.apply(grant, meta(T1, 2)).unwrap();
    let (executing, permit, _) = approved.authorize(verdict.plan(), meta(T1, 2)).unwrap();

    let refusal = executing
        .authorize(verdict.plan(), meta(T1, 3))
        .unwrap_err();
    assert!(matches!(
        refusal.error,
        TransitionError::InvalidTransition { .. }
    ));
    let (executed, _) = refusal
        .action
        .apply(Command::Executed { permit }, meta(T1, 3))
        .unwrap();
    assert_eq!(executed.state(), &State::Executed);
    let refusal = executed
        .apply(
            Command::Fail {
                error_code: ads_core::gate::ErrorCode::new("E_PLAN_CHANGED").unwrap(),
            },
            meta(T1, 4),
        )
        .unwrap_err();
    assert!(matches!(
        refusal.error,
        TransitionError::InvalidTransition { .. }
    ));
}

/// Two actions with the same plan both took the same grant.
#[test]
fn approval_of_one_action_does_not_fit_another_action_of_the_same_plan() {
    let verdict = unmeasured_delete();
    let (first, second) = (waiting("act-01", &verdict), waiting("act-02", &verdict));
    assert_eq!(first.plan_hash(), second.plan_hash());
    let hash = first.plan_hash().clone();

    // The decision taken for `act-01`, presented to both.
    let grant = || Command::Grant {
        action_id: id("act-01"),
        approval_id: id("appr-01"),
        plan_hash: hash.clone(),
        via: Via::LocalUi,
    };
    let (deny, expire) = (
        Command::Deny {
            action_id: id("act-01"),
            approval_id: id("appr-01"),
            plan_hash: hash.clone(),
            reason: None,
        },
        Command::Expire {
            action_id: id("act-01"),
            approval_id: id("appr-01"),
            plan_hash: hash.clone(),
        },
    );
    let mut second = second;
    for decision in [grant(), deny, expire] {
        let refusal = second.apply(decision, meta(EXPIRES, 2)).unwrap_err();
        assert_eq!(refusal.error, TransitionError::ActionMismatch);
        second = refusal.action;
    }
    assert!(matches!(second.state(), State::AwaitingApproval { .. }));
    let command = grant();
    let (first, event) = first.apply(command, meta(T1, 2)).unwrap();
    assert!(matches!(first.state(), State::Approved { .. }));
    assert_eq!(event.stream(), "approval/appr-01");
}

/// An allowed root inside another root could be deleted.
#[test]
fn allowed_root_inside_another_root_is_not_a_target() {
    let policy = policy(POSIX_HOME, "allowed_roots = ['/work/shared/data']")
        .with_work_roots(vec![AbsPath::parse("/work").unwrap()]);
    for target in ["/work/shared/data", "/work/shared", "/work/SHARED/*"] {
        for operation in [Op::DeleteRecursive, Op::Overwrite, Op::CleanUntracked] {
            let asked = request(operation, "/work", &[target]);
            let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
            assert_eq!(verdict.decision(), Decision::Denied, "{target}");
            assert!(verdict.has(Rule::AllowedRootItself), "{target}");
            assert!(!verdict.is_quarantined());
        }
    }
    // What is inside the allowed root, or beside it, is judged as before.
    for target in ["/work/shared/data/cache", "/work/shared/other"] {
        let asked = request(Op::DeleteRecursive, "/work", &[target]);
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert_eq!(verdict.decision(), Decision::Allowed, "{target}");
    }
}

/// A limit of zero with an estimate of zero passed as allowed.
#[test]
fn limit_of_zero_always_asks_for_approval() {
    let policy = policy(
        POSIX_HOME,
        "approval_item_limit = 0\napproval_byte_limit = 0",
    );
    for operation in [Op::DeleteRecursive, Op::CleanUntracked] {
        let mut asked = request(operation, "/work/proj", &["/work/proj/build"]);
        asked.estimate = Some(Estimate { items: 0, bytes: 0 });
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert_eq!(verdict.decision(), Decision::RequiresApproval);
        assert!(verdict.has(Rule::ItemLimitExceeded));
        assert!(verdict.has(Rule::ByteLimitExceeded));
    }
    // One limit at zero is enough, and the other keeps counting.
    let policy = common::policy(POSIX_HOME, "approval_byte_limit = 0");
    let mut asked = request(Op::DeleteRecursive, "/work/proj", &["/work/proj/build"]);
    asked.estimate = Some(Estimate { items: 0, bytes: 0 });
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
    assert!(!verdict.has(Rule::ItemLimitExceeded));
    assert!(verdict.has(Rule::ByteLimitExceeded));
}

/// Names that only a Unicode case fold makes equal (`K` and the
/// Kelvin sign U+212A, both `k` in lower case) are different folders. A sibling of a root
/// spelled that way was taken for the root, and a permit came out for a target outside it.
#[test]
fn unicode_look_alike_of_a_root_is_outside_it() {
    let root = "C:\\work\\K";
    let policy = policy(WIN_HOME, "allowed_roots = ['D:\\data\\K']").with_work_roots(
        [root, "C:\\work\\\u{C5}"]
            .map(|path| AbsPath::parse(path).unwrap())
            .to_vec(),
    );
    let judge = |operation: Op, target: &str| {
        let asked = request(operation, root, &[target]);
        classify(&ActionPlan::normalize(&asked).unwrap(), &policy)
    };

    // The target in the sibling: no permit.
    let verdict = judge(Op::Overwrite, "C:\\work\\\u{212A}\\outside.txt");
    assert_eq!(verdict.decision(), Decision::Denied);
    assert!(verdict.has(Rule::OutsideAllowedRoots));
    let (action, _, _) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    assert_eq!(action.state(), &State::Denied);
    assert!(action.authorize(verdict.plan(), meta(T1, 2)).is_err());

    // The same for every operation, for the Angstrom sign against the letter, for a
    // wildcard, for the destination of a move and for an allowed root.
    for operation in [Op::Delete, Op::DeleteRecursive, Op::CleanUntracked] {
        let verdict = judge(operation, "C:\\work\\\u{212A}\\*");
        assert_eq!(verdict.decision(), Decision::Denied);
        assert!(!verdict.is_quarantined());
    }
    let asked = request(
        Op::Overwrite,
        "C:\\work\\\u{C5}",
        &["C:\\work\\\u{212B}\\x"],
    );
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
    assert_eq!(verdict.decision(), Decision::Denied);
    let into_sibling = Op::Move {
        destination: "C:\\work\\\u{212A}\\taken".to_owned(),
    };
    let verdict = judge(into_sibling, "C:\\work\\K\\file.txt");
    assert_eq!(verdict.decision(), Decision::Denied);
    assert!(verdict.has(Rule::DestinationOutsideAllowedRoots));
    let verdict = judge(Op::Overwrite, "D:\\data\\\u{212A}\\file.txt");
    assert_eq!(verdict.decision(), Decision::Denied);
    assert!(verdict.has(Rule::OutsideAllowedRoots));

    // Plain ASCII case still is the same folder on Windows, in the three places.
    assert_eq!(
        judge(Op::Overwrite, "c:\\WORK\\k\\file.txt").decision(),
        Decision::Allowed
    );
    let inside = Op::Move {
        destination: "C:\\WORK\\k\\kept".to_owned(),
    };
    assert_eq!(
        judge(inside, "C:\\work\\K\\file.txt").decision(),
        Decision::Allowed
    );
    assert_eq!(
        judge(Op::Overwrite, "D:\\DATA\\k\\file.txt").decision(),
        Decision::RequiresApproval
    );

    // The deny rules keep the wide comparison: the look-alike of the work root, of the user
    // home and of a system folder is still treated as the protected one.
    assert!(judge(Op::Overwrite, "C:\\work\\\u{212A}").has(Rule::WorkRootItself));
    let home = common::policy("C:\\Users\\K", "");
    let asked = request(Op::Overwrite, "C:\\work", &["C:\\Users\\\u{212A}"]);
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &home);
    assert!(verdict.has(Rule::Protected(ads_core::gate::Protection::UserHome)));
    let system = common::policy(WIN_HOME, "windows_system_dirs = ['Kernel']");
    let asked = request(Op::Overwrite, "C:\\work", &["C:\\\u{212A}ernel\\x"]);
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &system);
    assert!(verdict.has(Rule::Protected(ads_core::gate::Protection::SystemDir)));
}

/// Instants that do not exist were accepted, against the contract.
#[test]
fn timestamp_must_be_a_real_instant() {
    for bad in [
        "2030-02-31T03:04:05Z",
        "2031-02-29T03:04:05Z",
        "2100-02-29T03:04:05Z",
        "2030-04-31T03:04:05Z",
        "2030-00-10T03:04:05Z",
        "2030-01-00T03:04:05Z",
        "2030-01-02T03:04:60Z",
        "2030-01-02T23:58:60Z",
        "2030-01-02T22:59:60Z",
        "2030-01-02T03:04:61Z",
    ] {
        assert!(Timestamp::new(bad).is_err(), "{bad}");
    }
    for good in [
        "2028-02-29T03:04:05Z",
        "2000-02-29T03:04:05Z",
        "2030-02-28T23:59:59.999999Z",
        "2030-12-31T23:59:60Z",
        "2030-01-31T00:00:00Z",
    ] {
        assert!(Timestamp::new(good).is_ok(), "{good}");
    }

    // An event dated on a day that does not exist does not read as an event.
    let verdict = unmeasured_delete();
    let (_, event, _) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    let mut json = event.to_json();
    assert!(ads_core::gate::Event::from_json(json.clone()).is_ok());
    json["ts"] = "2030-02-31T03:04:05Z".into();
    assert!(ads_core::gate::Event::from_json(json).is_err());
    // Nor does an approval valid until such a day.
    assert!(Timestamp::new("2030-02-30T00:00:00Z").is_err());
}

// ---- Second review ----------------------------------------------------------------------------

/// The rewrite to quarantine was not judged before the verdict was closed: a plain delete
/// nobody measured came out allowed, and the very plan of that verdict, judged again, asked
/// for approval. A permit was handed out for a plan the gate itself would not let through.
#[test]
fn verdict_already_holds_what_judging_its_plan_again_finds() {
    let policy = policy(POSIX_HOME, "");
    let judged_twice = |estimate: Option<Estimate>| {
        let mut asked = request(Op::Delete, "/work/proj", &["/work/proj/build"]);
        asked.estimate = estimate;
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        let again = classify(verdict.plan(), &policy);
        assert!(verdict.is_quarantined());
        assert_eq!(again.decision(), verdict.decision());
        for finding in again.findings() {
            assert!(verdict.findings().contains(finding), "{finding:?}");
        }
        verdict
    };

    let verdict = judged_twice(None);
    assert_eq!(verdict.decision(), Decision::RequiresApproval);
    assert!(verdict.has(Rule::EstimateUnknown));
    let (action, _, record) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    assert!(action.needs_approval());
    let refusal = action.authorize(verdict.plan(), meta(T1, 2)).unwrap_err();
    assert_eq!(refusal.error, TransitionError::ApprovalRequired);
    // The record tells the operation asked from the reason it needs approval.
    let record = record.to_json();
    assert_eq!(record["operation"], "fs.delete");
    assert_eq!(record["quarantined"], true);
    assert_eq!(record["verdict"]["reasons"][0]["rule"], "estimate_unknown");

    let above = judged_twice(Some(Estimate {
        items: 501,
        bytes: 1,
    }));
    assert_eq!(above.decision(), Decision::RequiresApproval);
    assert!(above.has(Rule::ItemLimitExceeded));
    // Measured and under the limits it is allowed, as before.
    let small = judged_twice(Some(Estimate {
        items: 3,
        bytes: 4096,
    }));
    assert_eq!(small.decision(), Decision::Allowed);
    assert!(small.findings().is_empty());
}

/// The destination the rewrite generates did not go through the rules of a destination. Here
/// the entry the move would create in the quarantine is an allowed root: the delete came out
/// allowed, rewritten as a move onto that root.
#[test]
fn generated_quarantine_destination_goes_through_the_destination_rules() {
    for (quarantine_dir, overrides) in [
        (".ads/quarantine", String::new()),
        ("trash/keep", "quarantine_dir = 'trash/keep'\n".to_owned()),
    ] {
        let asked = ActionPlan::normalize(&request(
            Op::DeleteRecursive,
            "/work/proj",
            &["/work/proj/build"],
        ))
        .unwrap();
        let folder = asked.hash().as_str()["sha256:".len()..][..16].to_owned();
        let entry = format!("/work/proj/{quarantine_dir}/{folder}/build");

        // Without that root the delete is rewritten and allowed.
        let plain = classify(&asked, &common::policy(POSIX_HOME, &overrides));
        assert_eq!(plain.decision(), Decision::Allowed);
        assert!(plain.is_quarantined());
        assert_eq!(
            plain.plan().destination().unwrap().to_string(),
            format!("/work/proj/{quarantine_dir}/{folder}")
        );

        let overrides = format!("{overrides}allowed_roots = ['{entry}']");
        let policy = common::policy(POSIX_HOME, &overrides);
        let verdict = classify(&asked, &policy);
        assert_eq!(verdict.decision(), Decision::Denied, "{quarantine_dir}");
        let reserved = verdict.findings().iter().any(|finding| {
            finding.rule == Rule::DestinationEntryReserved && finding.subject == entry
        });
        assert!(reserved, "{:?}", verdict.findings());
        // A denied plan is not rewritten: it stays the plan that was asked.
        assert!(!verdict.is_quarantined());
        assert_eq!(verdict.plan(), &asked);
        let (action, event, record) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
        assert_eq!(action.state(), &State::Denied);
        assert_eq!(
            event.to_json()["data"]["rules"],
            serde_json::json!(["destination_entry_reserved"])
        );
        assert_eq!(record.to_json()["destination"], serde_json::Value::Null);
        assert!(action.authorize(verdict.plan(), meta(T1, 2)).is_err());
        // Judged again, the same verdict.
        let again = classify(verdict.plan(), &policy);
        assert_eq!(again.decision(), Decision::Denied);
        assert_eq!(again.findings(), verdict.findings());
    }
}

/// The load-time side of the example of the review: a quarantine configured inside a
/// configured system folder loaded as a valid policy. What the load refuses is what can be
/// told with no work root, in the style of the policy; a name merely shared with a protected
/// folder of the other style, or of another root, is a common folder and loads.
#[test]
fn quarantine_configured_in_a_protected_folder_is_refused_at_load() {
    use ads_core::gate::{GatePolicy, HostFacts, PolicyError};
    let load = |home: &'static str, overrides: &str| {
        GatePolicy::from_toml_str(overrides, &HostFacts { user_home: home })
    };
    let refused = |home: &'static str, overrides: &str, dir: &str| {
        assert_eq!(
            load(home, overrides),
            Err(PolicyError::Invalid {
                key: "quarantine_dir",
                value: dir.to_owned()
            }),
            "{overrides}"
        );
    };

    refused(
        POSIX_HOME,
        "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
        "trash",
    );
    // The protected folder ends where the quarantine sub-path begins, at any depth, in any
    // case, for system folders and for the folders that hold the user profiles.
    refused(
        POSIX_HOME,
        "posix_system_dirs = ['/srv/vault']\nquarantine_dir = 'vault/q'",
        "vault/q",
    );
    refused(
        POSIX_HOME,
        "posix_system_dirs = ['/srv/Vault/Keep']\nquarantine_dir = 'vault/keep/q'",
        "vault/keep/q",
    );
    refused(
        POSIX_HOME,
        "posix_user_parents = ['/srv/people']\nquarantine_dir = 'people/q'",
        "people/q",
    );
    // A name Windows takes for a device is not a folder there, quarantine on or off.
    refused(WIN_HOME, "quarantine_dir = 'nul'", "nul");
    refused(WIN_HOME, "quarantine_dir = 'q/COM1.d'", "q/COM1.d");
    refused(
        WIN_HOME,
        "quarantine_enabled = false\nquarantine_dir = 'nul'",
        "nul",
    );
    // Outside the work root: the sub-path is relative and has no way up.
    for outside in ["../q", "q/../../x", "/abs/q", "C:/q", "q/./x", "q//x", ""] {
        let overrides = format!("quarantine_dir = '{outside}'");
        assert!(load(POSIX_HOME, &overrides).is_err(), "{outside}");
        assert!(load(WIN_HOME, &overrides).is_err(), "{outside}");
    }

    // Safe configurations that share a name with a protected folder: of the other style, of
    // the first level only (another root than the work root), beside it, or with quarantine
    // turned off. Each loads, and the rewrite it gives is allowed and lands in the work root.
    let cases: [(&str, &str, &str, &str); 12] = [
        (
            POSIX_HOME,
            "/work/proj",
            "quarantine_dir = 'trash'",
            "trash",
        ),
        (
            POSIX_HOME,
            "/work/proj",
            "quarantine_dir = 'etc/q'",
            "etc/q",
        ),
        (
            POSIX_HOME,
            "/work/proj",
            "quarantine_dir = 'home/q'",
            "home/q",
        ),
        (
            POSIX_HOME,
            "/work/proj",
            "quarantine_dir = 'Windows/q'",
            "Windows/q",
        ),
        (POSIX_HOME, "/work/proj", "quarantine_dir = 'nul'", "nul"),
        (
            POSIX_HOME,
            "/work/proj",
            "quarantine_dir = 'tmp/etc.d/q'",
            "tmp/etc.d/q",
        ),
        (
            POSIX_HOME,
            "/work/proj",
            "posix_system_dirs = ['/srv/vault']\nquarantine_dir = 'q/vault'",
            "q/vault",
        ),
        (
            WIN_HOME,
            "C:\\work\\proj",
            "quarantine_dir = 'etc/q'",
            "etc\\q",
        ),
        (
            WIN_HOME,
            "C:\\work\\proj",
            "quarantine_dir = 'home/q'",
            "home\\q",
        ),
        (
            WIN_HOME,
            "C:\\work\\proj",
            "quarantine_dir = 'Windows/q'",
            "Windows\\q",
        ),
        (
            WIN_HOME,
            "C:\\work\\proj",
            "quarantine_dir = 'users/q'",
            "users\\q",
        ),
        (
            WIN_HOME,
            "D:\\work\\proj",
            "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'",
            "trash",
        ),
    ];
    for (home, root, overrides, dir) in cases {
        let separator = if home == WIN_HOME { "\\" } else { "/" };
        let policy = load(home, overrides)
            .unwrap_or_else(|err| panic!("{overrides}: {err}"))
            .with_work_roots(vec![AbsPath::parse(root).unwrap()]);
        let target = format!("{root}{separator}build");
        let asked = request(Op::DeleteRecursive, root, &[&target]);
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert_eq!(verdict.decision(), Decision::Allowed, "{overrides}");
        assert!(verdict.is_quarantined(), "{overrides}");
        let destination = verdict.plan().destination().unwrap().to_string();
        assert!(
            destination.starts_with(&format!("{root}{separator}{dir}{separator}")),
            "{destination}"
        );
        // The quarantine of the work root is protected as a target, wherever it sits.
        let itself = format!("{root}{separator}{dir}");
        let asked = request(Op::DeleteRecursive, root, &[&itself]);
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert!(verdict.has(Rule::QuarantineItself), "{overrides}");

        // The same with quarantine off: it loads, and the delete is the delete.
        let off = format!("quarantine_enabled = false\n{overrides}");
        let policy = load(home, &off)
            .unwrap_or_else(|err| panic!("{off}: {err}"))
            .with_work_roots(vec![AbsPath::parse(root).unwrap()]);
        let asked = request(Op::DeleteRecursive, root, &[&target]);
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert_eq!(verdict.decision(), Decision::Allowed, "{off}");
        assert!(!verdict.is_quarantined(), "{off}");
    }
}

/// The literal example of the review, whatever the load says about the quarantine: the work
/// root is an ancestor of a configured system folder, so the plan is denied for the root
/// itself. Shown with configurations that load: the default quarantine, the quarantine of
/// the example turned off, and that system folder beside a quarantine of another name.
#[test]
fn work_root_that_holds_a_system_folder_denies_the_plan_by_itself() {
    use ads_core::gate::Protection;
    for overrides in [
        "posix_system_dirs = ['/work/proj/trash']",
        "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'trash'\nquarantine_enabled = false",
        "posix_system_dirs = ['/work/proj/trash']\nquarantine_dir = 'bin'",
        "posix_system_dirs = ['/work/proj/trash/deep']\nquarantine_dir = 'trash'",
    ] {
        let policy = policy(POSIX_HOME, overrides);
        for operation in [Op::Delete, Op::DeleteRecursive] {
            let asked = request(operation, "/work/proj", &["/work/proj/build"]);
            let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
            assert_eq!(verdict.decision(), Decision::Denied, "{overrides}");
            assert!(
                verdict.has(Rule::WorkRootProtected(Protection::SystemDir)),
                "{overrides}"
            );
            assert!(!verdict.is_quarantined(), "{overrides}");
            let (action, _, _) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
            assert_eq!(action.state(), &State::Denied);
            assert!(action.authorize(verdict.plan(), meta(T1, 2)).is_err());
        }
    }
}

// The variant the review asked for: two entries of `allowed_roots`, one inside the other.
#[test]
fn allowed_root_nested_in_another_allowed_root_is_not_a_target() {
    let policy = policy(
        POSIX_HOME,
        "allowed_roots = ['/data/shared', '/data/shared/inner']",
    );
    let judge = |operation: Op, target: &str| {
        let asked = request(operation, "/work/proj", &[target]);
        classify(&ActionPlan::normalize(&asked).unwrap(), &policy)
    };
    for target in [
        "/data/shared/inner",
        "/data/shared/INNER",
        "/data/shared/inner/",
        "/data/shared/*",
        "/data/shared",
        "/data",
    ] {
        for operation in [
            Op::Delete,
            Op::DeleteRecursive,
            Op::Overwrite,
            Op::CleanUntracked,
        ] {
            let verdict = judge(operation, target);
            assert_eq!(verdict.decision(), Decision::Denied, "{target}");
            assert!(verdict.has(Rule::AllowedRootItself), "{target}");
            assert!(!verdict.is_quarantined(), "{target}");
        }
    }
    // Inside the inner root, and inside the outer one beside the inner, it only asks.
    for target in ["/data/shared/inner/cache", "/data/shared/other"] {
        let verdict = judge(Op::Overwrite, target);
        assert_eq!(verdict.decision(), Decision::RequiresApproval, "{target}");
        assert!(verdict.has(Rule::OutsideWorkRoot));
    }
    // Nor is the inner root the place a move lands on.
    let onto = Op::Move {
        destination: "/data/shared".to_owned(),
    };
    let verdict = judge(onto, "/data/shared/tmp/inner");
    assert_eq!(verdict.decision(), Decision::Denied);
    assert!(verdict.has(Rule::DestinationEntryReserved));
}

/// The second conclusion the isolated test of the first review left out: an executed action
/// does not take `Executed` again, even with a permit that names it.
#[test]
fn executed_action_is_not_concluded_twice() {
    let verdict = unmeasured_delete();
    let ready = |action: &str| {
        let waiting = waiting(action, &verdict);
        let grant = Command::Grant {
            action_id: id(action),
            approval_id: id("appr-01"),
            plan_hash: waiting.plan_hash().clone(),
            via: Via::LocalUi,
        };
        let (approved, _) = waiting.apply(grant, meta(T1, 2)).unwrap();
        approved.authorize(verdict.plan(), meta(T1, 2)).unwrap()
    };
    let (executing, permit, _) = ready("act-01");
    // The same action planned again by another caller: the same id, plan and approval, so
    // its permit is equal in every field to the one already used.
    let (_, twin, _) = ready("act-01");
    assert_eq!(permit, twin);

    let (executed, _) = executing
        .apply(Command::Executed { permit }, meta(T1, 3))
        .unwrap();
    assert_eq!(executed.state(), &State::Executed);
    let refusal = executed
        .apply(Command::Executed { permit: twin }, meta(T1, 4))
        .unwrap_err();
    assert!(matches!(
        refusal.error,
        TransitionError::InvalidTransition {
            state: State::Executed,
            command: "executed"
        }
    ));
    assert_eq!(refusal.action.state(), &State::Executed);
}

/// NFC and NFD spellings, full-width forms and the device names Windows writes with a
/// superscript digit, as explicit cases. Text is all the gate has: two spellings are two
/// names, and a name that is not the root never grants.
#[test]
fn unicode_spellings_of_a_root_never_grant() {
    use ads_core::gate::{PathError, PlanError};
    // U+00E9 against `e` and U+0301.
    let (nfc, nfd) = ("caf\u{e9}", "cafe\u{301}");
    for (root_name, other_name) in [(nfc, nfd), (nfd, nfc)] {
        for (root, other, home) in [
            (
                format!("/work/{root_name}"),
                format!("/work/{other_name}"),
                POSIX_HOME,
            ),
            (
                format!("C:\\work\\{root_name}"),
                format!("C:\\work\\{other_name}"),
                WIN_HOME,
            ),
        ] {
            let separator = if home == WIN_HOME { "\\" } else { "/" };
            let policy =
                common::policy(home, "").with_work_roots(vec![AbsPath::parse(&root).unwrap()]);
            let judge = |operation: Op, target: &str| {
                let asked = request(operation, &root, &[target]);
                classify(&ActionPlan::normalize(&asked).unwrap(), &policy)
            };
            let inside = format!("{root}{separator}build");
            let sibling = format!("{other}{separator}build");
            assert_eq!(judge(Op::Overwrite, &inside).decision(), Decision::Allowed);
            for operation in [
                Op::Delete,
                Op::DeleteRecursive,
                Op::Overwrite,
                Op::CleanUntracked,
            ] {
                let verdict = judge(operation, &sibling);
                assert_eq!(verdict.decision(), Decision::Denied, "{sibling}");
                assert!(verdict.has(Rule::OutsideAllowedRoots));
                assert!(!verdict.is_quarantined());
            }
            let into_sibling = Op::Move { destination: other };
            let verdict = judge(into_sibling, &inside);
            assert_eq!(verdict.decision(), Decision::Denied);
            assert!(verdict.has(Rule::DestinationOutsideAllowedRoots));
        }
    }
    // The same for an allowed root.
    let policy = common::policy(POSIX_HOME, "allowed_roots = ['/data/caf\u{e9}']");
    let asked = request(Op::Overwrite, "/work/proj", &["/data/cafe\u{301}/x"]);
    let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
    assert_eq!(verdict.decision(), Decision::Denied);

    // Full-width letters, separators and dots are plain characters of a name.
    let root = "C:\\work\\proj";
    let policy = common::policy(WIN_HOME, "");
    for outside in [
        "C:\\work\\\u{ff50}\u{ff52}\u{ff4f}\u{ff4a}\\x",
        "C:\\work\\proj\u{ff3c}x",
        "C:\\work\\proj\u{ff0f}x",
        "C:\\work\\\u{ff0e}\u{ff0e}\\proj\\x",
    ] {
        let asked = request(Op::Overwrite, root, &[outside]);
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert_eq!(verdict.decision(), Decision::Denied, "{outside}");
        assert!(verdict.has(Rule::OutsideAllowedRoots), "{outside}");
    }
    // A full-width drive letter or colon is no drive at all.
    for no_drive in ["\u{ff23}:\\work\\proj\\x", "C\u{ff1a}\\work\\proj\\x"] {
        let asked = request(Op::Overwrite, root, &[no_drive]);
        assert_eq!(
            ActionPlan::normalize(&asked),
            Err(PlanError::Target {
                index: 0,
                error: PathError::Relative
            }),
            "{no_drive}"
        );
    }

    // Device names: the superscript digits and zero count, in any case, with any extension.
    for device in [
        "COM\u{b9}",
        "COM\u{b2}",
        "com\u{b3}",
        "LPT\u{b9}",
        "lpt\u{b2}.txt",
        "LPT\u{b3}",
        "COM0",
        "LPT0",
        "COM\u{b9} .log",
        "NUL",
        "nul.tar.gz",
    ] {
        let target = format!("{root}\\{device}");
        for operation in [Op::Delete, Op::Overwrite] {
            let asked = request(operation, root, &[&target]);
            assert_eq!(
                ActionPlan::normalize(&asked),
                Err(PlanError::Target {
                    index: 0,
                    error: PathError::DeviceName
                }),
                "{device}"
            );
        }
        // Nor as the work root or as the destination of a move.
        let asked = request(Op::Overwrite, &target, &[&format!("{target}\\x")]);
        assert_eq!(
            ActionPlan::normalize(&asked),
            Err(PlanError::WorkRoot(PathError::DeviceName))
        );
        let to_device = Op::Move {
            destination: target.clone(),
        };
        let asked = request(to_device, root, &["C:\\work\\proj\\build"]);
        assert_eq!(
            ActionPlan::normalize(&asked),
            Err(PlanError::Destination(PathError::DeviceName))
        );
    }
    // What only looks like one is a plain name: other superscripts, full-width digits and
    // letters, two digits.
    for plain in [
        "COM\u{2074}",
        "COM\u{ff11}",
        "\u{ff2e}\u{ff35}\u{ff2c}",
        "COM10",
    ] {
        let target = format!("{root}\\{plain}");
        let asked = request(Op::Overwrite, root, &[&target]);
        let verdict = classify(&ActionPlan::normalize(&asked).unwrap(), &policy);
        assert_eq!(verdict.decision(), Decision::Allowed, "{plain}");
    }
}

/// The limit of comparing text, written down: the deny rules fold case, not Unicode
/// normalization. A protected folder spelled in the other normalization form is, as text,
/// another folder, and only whoever resolves the path on disk can tell. Where the usual rules
/// still catch it (a child of the folder of the profiles) nothing is lost; a user home
/// elsewhere is the case text alone does not cover.
#[test]
fn deny_rules_compare_text_and_not_unicode_normalization() {
    use ads_core::gate::{AbsPath, Protection};
    let protection = |home: &str, path: &str| {
        common::policy(home, "").protection(&AbsPath::parse(path).unwrap())
    };
    let (nfc, nfd) = ("/home/jos\u{e9}", "/home/jose\u{301}");
    assert_eq!(protection(nfc, nfc), Some(Protection::UserHome));
    assert_eq!(protection(nfc, nfd), Some(Protection::UserProfile));
    let (nfc, nfd) = ("/data/people/jos\u{e9}", "/data/people/jose\u{301}");
    assert_eq!(protection(nfc, nfc), Some(Protection::UserHome));
    assert_eq!(protection(nfc, nfd), None);
}
