//! The state machine of an action: every legal path, and every jump it must refuse. A
//! transition consumes the action, so each refusal is checked on the action it gives back.

mod common;

use ads_core::gate::{
    Action, ActionPlan, Command, Decision, ErrorCode, Estimate, Event, ExecutionPermit,
    OperationRequest as Op, PlanHash, Refused, ShortText, State, TransitionError, Verdict, Via,
    classify,
};
use common::{WIN_HOME, WIN_ROOT, at, id, meta, plan, policy, request, win};

const T0: &str = "2030-01-02T03:04:05Z";
const T1: &str = "2030-01-02T03:06:05Z";
const EXPIRES: &str = "2030-01-02T03:19:05Z";
const AFTER: &str = "2030-01-02T03:19:05.000001Z";

const BUILD: &str = "C:\\Users\\user-a\\work\\proj\\build";

/// A small delete: allowed, rewritten to quarantine.
fn allowed() -> Verdict {
    let verdict = win(Op::Delete, &[BUILD]);
    assert_eq!(verdict.decision(), Decision::Allowed);
    verdict
}

/// A recursive delete with no estimate: requires approval.
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

fn planned(verdict: &Verdict) -> Action {
    Action::plan(id("act-01"), verdict, meta(T0, 1)).unwrap().0
}

fn ask(approval: &str) -> Command {
    Command::RequestApproval {
        approval_id: id(approval),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at: at(EXPIRES),
    }
}

fn grant_of(action: &Action, approval: &str) -> Command {
    Command::Grant {
        action_id: action.action_id().clone(),
        approval_id: id(approval),
        plan_hash: action.plan_hash().clone(),
        via: Via::LocalUi,
    }
}

fn deny_of(action: &Action) -> Command {
    Command::Deny {
        action_id: action.action_id().clone(),
        approval_id: id("appr-01"),
        plan_hash: action.plan_hash().clone(),
        reason: None,
    }
}

fn expire_of(action: &Action) -> Command {
    Command::Expire {
        action_id: action.action_id().clone(),
        approval_id: id("appr-01"),
        plan_hash: action.plan_hash().clone(),
    }
}

fn request_approval(action: Action) -> Action {
    action.apply(ask("appr-01"), meta(T0, 1)).unwrap().0
}

fn grant(action: Action) -> Action {
    let command = grant_of(&action, "appr-01");
    action.apply(command, meta(T1, 2)).unwrap().0
}

fn waiting(verdict: &Verdict) -> Action {
    request_approval(planned(verdict))
}

fn approved(verdict: &Verdict) -> Action {
    grant(waiting(verdict))
}

fn executing(verdict: &Verdict) -> (Action, ExecutionPermit) {
    let action = if verdict.decision() == Decision::Allowed {
        planned(verdict)
    } else {
        approved(verdict)
    };
    let (action, permit, _) = action.authorize(verdict.plan(), meta(T1, 2)).unwrap();
    (action, permit)
}

fn fail() -> Command {
    Command::Fail {
        error_code: ErrorCode::new("E_PLAN_CHANGED").unwrap(),
    }
}

fn other_hash() -> PlanHash {
    PlanHash::new(format!("sha256:{}", "ab".repeat(32))).unwrap()
}

/// A permit for the same plan, taken from another action.
fn foreign_permit(verdict: &Verdict) -> ExecutionPermit {
    let (other, _, _) = Action::plan(id("act-02"), verdict, meta(T0, 1)).unwrap();
    other.authorize(verdict.plan(), meta(T1, 2)).unwrap().1
}

/// The refusal is `error`; gives back the action, which a refusal leaves as it was.
fn refused<T: std::fmt::Debug>(result: Result<T, Box<Refused>>, error: TransitionError) -> Action {
    let refusal = result.expect_err("accepted");
    assert_eq!(refusal.error, error);
    refusal.action
}

/// The refusal is "this does not fit the state"; gives back the action.
fn out_of_place<T: std::fmt::Debug>(result: Result<T, Box<Refused>>) -> Action {
    let refusal = result.expect_err("accepted");
    assert!(
        matches!(refusal.error, TransitionError::InvalidTransition { .. }),
        "{:?}",
        refusal.error
    );
    refusal.action
}

#[test]
fn allowed_plan_runs_without_approval() {
    let verdict = allowed();
    let (action, event, _) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    assert_eq!(action.state(), &State::Planned);
    assert!(!action.needs_approval());
    assert_eq!(action.plan_hash(), &verdict.plan().hash());
    let Event::ActionPlanned(event) = event else {
        panic!("not action.planned");
    };
    assert_eq!(event.stream(), "action/act-01");
    assert_eq!(event.data().verb.as_str(), "fs.move");
    assert_eq!(event.data().plan_hash, verdict.plan().hash());
    assert!(!event.data().needs_elevation);

    let (action, permit, event) = action.authorize(verdict.plan(), meta(T1, 2)).unwrap();
    assert_eq!(action.state(), &State::Executing { approval_id: None });
    assert_eq!(permit.plan(), verdict.plan());
    assert_eq!(permit.approval_id(), None);
    let Event::ActionStarted(event) = event else {
        panic!("not action.started");
    };
    assert_eq!(event.stream(), "action/act-01");
    assert_eq!(event.data().approval_id, None);
    assert_eq!(&event.data().plan_hash, action.plan_hash());

    let (action, event) = action
        .apply(Command::Executed { permit }, meta(T1, 3))
        .unwrap();
    assert_eq!(action.state(), &State::Executed);
    let Event::ActionExecuted(event) = event else {
        panic!("not action.executed");
    };
    assert_eq!(event.data().approval_id, None);
    assert_eq!(&event.data().plan_hash, action.plan_hash());
}

#[test]
fn approved_plan_runs_with_its_approval() {
    let verdict = approval_needed(&[BUILD]);
    let action = planned(&verdict);
    assert!(action.needs_approval());

    let action = request_approval(action);
    assert!(matches!(action.state(), State::AwaitingApproval { .. }));
    let action = grant(action);
    assert!(matches!(action.state(), State::Approved { .. }));

    let (action, permit, event) = action.authorize(verdict.plan(), meta(T1, 2)).unwrap();
    assert_eq!(permit.approval_id(), Some(&id("appr-01")));
    let Event::ActionStarted(event) = event else {
        panic!("not action.started");
    };
    assert_eq!(event.data().approval_id, Some(id("appr-01")));
    let (action, event) = action
        .apply(Command::Executed { permit }, meta(T1, 3))
        .unwrap();
    assert_eq!(action.state(), &State::Executed);
    let Event::ActionExecuted(event) = event else {
        panic!("not action.executed");
    };
    assert_eq!(event.data().approval_id, Some(id("appr-01")));
}

#[test]
fn denied_verdict_emits_action_denied_and_ends_the_action() {
    let verdict = win(Op::DeleteRecursive, &[WIN_HOME]);
    assert_eq!(verdict.decision(), Decision::Denied);
    let (action, event, record) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    assert_eq!(action.state(), &State::Denied);
    assert!(action.state().is_terminal());
    let Event::ActionDenied(event) = event else {
        panic!("not action.denied");
    };
    assert_eq!(event.stream(), "action/act-01");
    assert_eq!(event.data().verb.as_str(), "fs.delete_recursive");
    assert_eq!(event.data().plan_hash, verdict.plan().hash());
    let rules: Vec<&str> = event
        .data()
        .rules
        .as_slice()
        .iter()
        .map(|rule| rule.as_str())
        .collect();
    assert_eq!(
        rules,
        [
            "protected.user_home",
            "work_root_itself",
            "outside_allowed_roots",
            "quarantine_itself"
        ]
    );
    assert_eq!(record.fields().verdict.decision, Decision::Denied);
    assert!(!record.fields().quarantined);

    // Nothing comes out of a denied action: no permit, no approval, not even a failure.
    let action = out_of_place(action.authorize(verdict.plan(), meta(T1, 2)));
    let action = out_of_place(action.apply(ask("appr-01"), meta(T1, 2)));
    let command = grant_of(&action, "appr-01");
    let action = out_of_place(action.apply(command, meta(T1, 2)));
    let action = out_of_place(action.apply(fail(), meta(T1, 2)));
    assert_eq!(action.state(), &State::Denied);
}

#[test]
fn denied_event_lists_each_denying_rule_once_and_no_approval_rule() {
    // Two targets outside every root, with no estimate: the same deny rule fires twice and an
    // approval rule fires too.
    let mut request = request(
        Op::DeleteRecursive,
        WIN_ROOT,
        &["D:\\other\\a", "D:\\other\\b"],
    );
    request.estimate = None;
    let verdict = classify(
        &ActionPlan::normalize(&request).unwrap(),
        &policy(WIN_HOME, ""),
    );
    let fired: Vec<String> = verdict
        .findings()
        .iter()
        .map(|finding| finding.rule.code().to_string())
        .collect();
    assert_eq!(
        fired,
        [
            "outside_allowed_roots",
            "outside_work_root",
            "outside_allowed_roots",
            "outside_work_root",
            "estimate_unknown"
        ]
    );
    let (_, event, record) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
    assert_eq!(
        event.to_json()["data"]["rules"],
        serde_json::json!(["outside_allowed_roots"])
    );
    // The record keeps every rule that fired, with the path it fired on.
    assert_eq!(record.fields().verdict.reasons.len(), 5);
}

#[test]
fn plan_that_does_not_fit_the_record_starts_nothing() {
    let long = format!("{WIN_ROOT}\\{}", "a".repeat(4096));
    let verdict = win(Op::Overwrite, &[&long]);
    assert_eq!(verdict.decision(), Decision::Allowed);
    assert!(matches!(
        Action::plan(id("act-01"), &verdict, meta(T0, 1)),
        Err(TransitionError::Unrecordable(_))
    ));
}

#[test]
fn execution_without_the_required_approval_is_refused() {
    let verdict = approval_needed(&[BUILD]);
    let action = refused(
        planned(&verdict).authorize(verdict.plan(), meta(T1, 2)),
        TransitionError::ApprovalRequired,
    );
    // No permit, no `executed`: the command needs a permit and the state is not `Executing`.
    let stolen = foreign_permit(&allowed());
    let action = out_of_place(action.apply(Command::Executed { permit: stolen }, meta(T1, 2)));
    let waiting = request_approval(action);
    let waiting = refused(
        waiting.authorize(verdict.plan(), meta(T1, 2)),
        TransitionError::ApprovalRequired,
    );
    assert!(matches!(waiting.state(), State::AwaitingApproval { .. }));
}

#[test]
fn approval_reused_after_changing_a_target_is_refused() {
    let approved_verdict = approval_needed(&[BUILD]);
    let approved_action = approved(&approved_verdict);
    let approved_hash = approved_action.plan_hash().clone();

    // The same request with one more target: another plan, another hash.
    let changed_verdict = approval_needed(&[BUILD, "C:\\Users\\user-a\\work\\proj\\src"]);
    assert_ne!(changed_verdict.plan().hash(), approved_hash);

    // 1. The approved action does not authorize the changed plan.
    let approved_action = refused(
        approved_action.authorize(changed_verdict.plan(), meta(T1, 2)),
        TransitionError::PlanHashMismatch,
    );

    // 2. The changed plan is a new action, and the old approval does not fit it: neither
    //    carrying the old hash, nor relabelled with the new one.
    let changed_action = refused(
        planned(&changed_verdict).authorize(changed_verdict.plan(), meta(T1, 2)),
        TransitionError::ApprovalRequired,
    );
    let old_grant = || Command::Grant {
        action_id: id("act-01"),
        approval_id: id("appr-01"),
        plan_hash: approved_hash.clone(),
        via: Via::LocalUi,
    };
    let changed_action = out_of_place(changed_action.apply(old_grant(), meta(T1, 2)));
    let changed_waiting = changed_action.apply(ask("appr-02"), meta(T0, 1)).unwrap().0;
    let changed_waiting = refused(
        changed_waiting.apply(old_grant(), meta(T1, 2)),
        TransitionError::PlanHashMismatch,
    );
    let relabelled = grant_of(&changed_waiting, "appr-01");
    let changed_waiting = refused(
        changed_waiting.apply(relabelled, meta(T1, 2)),
        TransitionError::ApprovalMismatch,
    );

    // 3. The permit of the approved plan does not close the changed action.
    let (_, old_permit, _) = approved_action
        .authorize(approved_verdict.plan(), meta(T1, 2))
        .unwrap();
    let command = grant_of(&changed_waiting, "appr-02");
    let changed_approved = changed_waiting.apply(command, meta(T1, 2)).unwrap().0;
    let (changed_executing, _, _) = changed_approved
        .authorize(changed_verdict.plan(), meta(T1, 2))
        .unwrap();
    refused(
        changed_executing.apply(Command::Executed { permit: old_permit }, meta(T1, 3)),
        TransitionError::PlanHashMismatch,
    );
}

#[test]
fn changing_any_part_of_the_plan_invalidates_the_permit_check() {
    let verdict = allowed();
    let mut action = planned(&verdict);
    let variants = [
        plan(
            Op::Delete,
            WIN_ROOT,
            &["C:\\Users\\user-a\\work\\proj\\src"],
        ),
        plan(
            Op::Delete,
            WIN_ROOT,
            &[BUILD, "C:\\Users\\user-a\\work\\proj\\src"],
        ),
        plan(Op::DeleteRecursive, WIN_ROOT, &[BUILD]),
        plan(Op::Delete, "C:\\Users\\user-a\\work", &[BUILD]),
        // The plan as asked, before the quarantine rewrite, is not the plan that was planned.
        plan(Op::Delete, WIN_ROOT, &[BUILD]),
    ];
    for other in variants {
        action = refused(
            action.authorize(&other, meta(T1, 2)),
            TransitionError::PlanHashMismatch,
        );
    }
    assert_eq!(action.state(), &State::Planned);
}

#[test]
fn approval_for_a_plan_that_needs_none_is_refused() {
    refused(
        planned(&allowed()).apply(ask("appr-01"), meta(T0, 1)),
        TransitionError::ApprovalNotRequired,
    );
}

#[test]
fn approval_cannot_be_born_expired() {
    let mut action = planned(&approval_needed(&[BUILD]));
    for expires_at in [T0, "2030-01-02T03:04:04.999Z"] {
        let command = Command::RequestApproval {
            approval_id: id("appr-01"),
            summary: ShortText::new("Apagar a pasta de build").unwrap(),
            expires_at: at(expires_at),
        };
        action = refused(
            action.apply(command, meta(T0, 1)),
            TransitionError::ExpiryNotInFuture,
        );
    }
}

#[test]
fn grant_needs_the_pending_approval_the_same_action_and_hash_and_time_left() {
    let waiting = waiting(&approval_needed(&[BUILD]));
    let hash = waiting.plan_hash().clone();
    let grant = |action: &str, approval: &str, hash: &PlanHash| Command::Grant {
        action_id: id(action),
        approval_id: id(approval),
        plan_hash: hash.clone(),
        via: Via::RemoteDevice,
    };
    let waiting = refused(
        waiting.apply(grant("act-01", "appr-99", &hash), meta(T1, 2)),
        TransitionError::ApprovalMismatch,
    );
    let waiting = refused(
        waiting.apply(grant("act-01", "appr-01", &other_hash()), meta(T1, 2)),
        TransitionError::PlanHashMismatch,
    );
    let mut waiting = refused(
        waiting.apply(grant("act-02", "appr-01", &hash), meta(T1, 2)),
        TransitionError::ActionMismatch,
    );
    for late in [EXPIRES, AFTER] {
        waiting = refused(
            waiting.apply(grant("act-01", "appr-01", &hash), meta(late, 2)),
            TransitionError::ApprovalLapsed,
        );
    }
    let (granted, event) = waiting
        .apply(grant("act-01", "appr-01", &hash), meta(T1, 2))
        .unwrap();
    assert!(matches!(granted.state(), State::Approved { .. }));
    let Event::ApprovalGranted(event) = event else {
        panic!("not approval.granted");
    };
    assert_eq!(event.stream(), "approval/appr-01");
    assert_eq!(event.data().via, Via::RemoteDevice);
}

#[test]
fn denied_approval_ends_the_action() {
    let verdict = approval_needed(&[BUILD]);
    let waiting = waiting(&verdict);
    let wrong = Command::Deny {
        action_id: id("act-01"),
        approval_id: id("appr-01"),
        plan_hash: other_hash(),
        reason: None,
    };
    let waiting = refused(
        waiting.apply(wrong, meta(T1, 2)),
        TransitionError::PlanHashMismatch,
    );
    let deny = Command::Deny {
        action_id: id("act-01"),
        approval_id: id("appr-01"),
        plan_hash: waiting.plan_hash().clone(),
        reason: Some(ShortText::new("fora de hora").unwrap()),
    };
    let (denied, event) = waiting.apply(deny, meta(T1, 2)).unwrap();
    assert_eq!(denied.state(), &State::ApprovalDenied);
    assert!(matches!(event, Event::ApprovalDenied(_)));
    out_of_place(denied.authorize(verdict.plan(), meta(T1, 2)));
}

#[test]
fn approval_expires_only_after_its_validity() {
    let verdict = approval_needed(&[BUILD]);
    let waiting = waiting(&verdict);
    let command = expire_of(&waiting);
    let waiting = refused(
        waiting.apply(command, meta(T1, 2)),
        TransitionError::NotYetExpired,
    );
    let command = expire_of(&waiting);
    let (expired, event) = waiting.apply(command, meta(EXPIRES, 2)).unwrap();
    assert_eq!(expired.state(), &State::ApprovalExpired);
    assert!(matches!(event, Event::ApprovalExpired(_)));
    out_of_place(expired.authorize(verdict.plan(), meta(AFTER, 2)));
}

#[test]
fn granted_approval_does_not_outlive_its_validity() {
    let verdict = approval_needed(&[BUILD]);
    let mut action = approved(&verdict);
    for late in [EXPIRES, AFTER] {
        action = refused(
            action.authorize(verdict.plan(), meta(late, 2)),
            TransitionError::ApprovalLapsed,
        );
    }
    // The way out is to fail the action and plan again.
    let (failed, event) = action.apply(fail(), meta(AFTER, 2)).unwrap();
    assert_eq!(failed.state(), &State::Failed);
    let Event::ActionFailed(event) = event else {
        panic!("not action.failed");
    };
    assert_eq!(event.data().error_code.as_str(), "E_PLAN_CHANGED");
    assert_eq!(event.stream(), "action/act-01");
}

#[test]
fn permit_is_single_use_and_bound_to_its_action() {
    let verdict = allowed();
    let (executing, permit) = executing(&verdict);
    // A second permit is not handed out.
    let executing = out_of_place(executing.authorize(verdict.plan(), meta(T1, 2)));
    // The permit of another action does not close this one.
    let executing = refused(
        executing.apply(
            Command::Executed {
                permit: foreign_permit(&verdict),
            },
            meta(T1, 2),
        ),
        TransitionError::ActionMismatch,
    );
    // Reporting the execution consumes the permit and the action: there is no second report
    // to make, and the action that comes out takes neither a permit request nor a failure.
    let (executed, _) = executing
        .apply(Command::Executed { permit }, meta(T1, 2))
        .unwrap();
    let executed = out_of_place(executed.authorize(verdict.plan(), meta(T1, 3)));
    let executed = out_of_place(executed.apply(fail(), meta(T1, 3)));
    let again = foreign_permit(&verdict);
    out_of_place(executed.apply(Command::Executed { permit: again }, meta(T1, 3)));
}

#[test]
fn one_approval_yields_one_permit() {
    let verdict = approval_needed(&[BUILD]);
    let approved = approved(&verdict);
    let (executing, permit, _) = approved.authorize(verdict.plan(), meta(T1, 2)).unwrap();
    // `approved` was consumed: the only action left is the one already executing, and it hands
    // out nothing more, with the approval still inside its validity.
    let executing = out_of_place(executing.authorize(verdict.plan(), meta(T1, 3)));
    let command = grant_of(&executing, "appr-01");
    let executing = out_of_place(executing.apply(command, meta(T1, 3)));
    let (executed, _) = executing
        .apply(Command::Executed { permit }, meta(T1, 3))
        .unwrap();
    assert_eq!(executed.state(), &State::Executed);
}

#[test]
fn execution_can_fail() {
    let (executing, _) = executing(&allowed());
    let (failed, event) = executing.apply(fail(), meta(T1, 2)).unwrap();
    assert_eq!(failed.state(), &State::Failed);
    assert!(matches!(event, Event::ActionFailed(_)));
}

#[test]
fn terminal_states_accept_nothing() {
    let verdict = approval_needed(&[BUILD]);
    let denied = {
        let waiting = waiting(&verdict);
        let command = deny_of(&waiting);
        waiting.apply(command, meta(T1, 2)).unwrap().0
    };
    let expired = {
        let waiting = waiting(&verdict);
        let command = expire_of(&waiting);
        waiting.apply(command, meta(AFTER, 2)).unwrap().0
    };
    let executed = {
        let (executing, permit) = executing(&verdict);
        executing
            .apply(Command::Executed { permit }, meta(T1, 2))
            .unwrap()
            .0
    };
    let failed = executing(&verdict).0.apply(fail(), meta(T1, 2)).unwrap().0;

    for mut end in [denied, expired, executed, failed] {
        assert!(end.state().is_terminal());
        let state = end.state().clone();
        let (_, spare) = executing(&verdict);
        let commands = [
            ask("appr-02"),
            grant_of(&end, "appr-01"),
            deny_of(&end),
            expire_of(&end),
            Command::Executed { permit: spare },
            fail(),
        ];
        for command in commands {
            end = out_of_place(end.apply(command, meta(AFTER, 9)));
        }
        end = out_of_place(end.authorize(verdict.plan(), meta(T1, 2)));
        assert_eq!(end.state(), &state);
    }
}

#[test]
fn commands_out_of_order_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    let mut action = planned(&verdict);
    // Decisions with no request.
    let decisions = [
        grant_of(&action, "appr-01"),
        deny_of(&action),
        expire_of(&action),
    ];
    for decision in decisions {
        action = out_of_place(action.apply(decision, meta(T1, 2)));
    }
    let waiting = request_approval(action);
    // A second request, or giving up, while one is pending.
    let waiting = out_of_place(waiting.apply(ask("appr-02"), meta(T1, 2)));
    let waiting = out_of_place(waiting.apply(fail(), meta(T1, 2)));
    // A second decision after the first.
    let granted = grant(waiting);
    let command = deny_of(&granted);
    let granted = out_of_place(granted.apply(command, meta(T1, 3)));
    assert!(matches!(granted.state(), State::Approved { .. }));
}

#[test]
fn estimate_is_part_of_what_was_approved() {
    let verdict = approval_needed(&[BUILD]);
    let action = approved(&verdict);
    let mut measured = request(Op::DeleteRecursive, WIN_ROOT, &[BUILD]);
    measured.estimate = Some(Estimate { items: 1, bytes: 1 });
    let measured = classify(
        &ActionPlan::normalize(&measured).unwrap(),
        &policy(WIN_HOME, ""),
    );
    refused(
        action.authorize(measured.plan(), meta(T1, 2)),
        TransitionError::PlanHashMismatch,
    );
}
