//! Regression tests for defects found in independent reviews of the gate. Each one failed
//! before its fix. Paths are synthetic text; nothing is touched.

mod common;

use std::marker::PhantomData;

use ads_core::gate::{
    Action, ActionPlan, Command, Decision, Estimate, ExecutionPermit, OperationRequest as Op, Rule,
    ShortText, State, Timestamp, TransitionError, Verdict, Via, classify,
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
    let policy = policy(POSIX_HOME, "allowed_roots = ['/work/shared/data']");
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
    let policy = policy(WIN_HOME, "allowed_roots = ['D:\\data\\K']");
    let root = "C:\\work\\K";
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
