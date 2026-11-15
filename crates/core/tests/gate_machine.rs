//! The state machine of an action: every legal path, and every jump it must refuse.

mod common;

use ads_core::gate::{
    Action, ActionPlan, Command, Decision, ErrorCode, Estimate, Event, ExecutionPermit,
    OperationRequest as Op, PlanHash, ShortText, State, TransitionError, Verdict, Via, classify,
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

fn request_approval(action: &Action) -> Action {
    let command = Command::RequestApproval {
        approval_id: id("appr-01"),
        summary: ShortText::new("Apagar a pasta de build").unwrap(),
        expires_at: at(EXPIRES),
    };
    action.apply(command, meta(T0, 1)).unwrap().0
}

fn grant(action: &Action) -> Action {
    let command = Command::Grant {
        approval_id: id("appr-01"),
        plan_hash: action.plan_hash().clone(),
        via: Via::LocalUi,
    };
    action.apply(command, meta(T1, 2)).unwrap().0
}

fn approved(verdict: &Verdict) -> Action {
    grant(&request_approval(&planned(verdict)))
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
    let (other, _) = Action::plan(id("act-02"), verdict, meta(T0, 1)).unwrap();
    other.authorize(verdict.plan(), &at(T1)).unwrap().1
}

#[test]
fn allowed_plan_runs_without_approval() {
    let verdict = allowed();
    let (action, event) = Action::plan(id("act-01"), &verdict, meta(T0, 1)).unwrap();
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

    let (action, permit) = action.authorize(verdict.plan(), &at(T1)).unwrap();
    assert_eq!(action.state(), &State::Executing { approval_id: None });
    assert_eq!(permit.plan(), verdict.plan());
    assert_eq!(permit.approval_id(), None);

    let (action, event) = action
        .apply(Command::Executed { permit }, meta(T1, 2))
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

    let action = request_approval(&action);
    assert!(matches!(action.state(), State::AwaitingApproval { .. }));
    let action = grant(&action);
    assert!(matches!(action.state(), State::Approved { .. }));

    let (action, permit) = action.authorize(verdict.plan(), &at(T1)).unwrap();
    assert_eq!(permit.approval_id(), Some(&id("appr-01")));
    let (action, event) = action
        .apply(Command::Executed { permit }, meta(T1, 2))
        .unwrap();
    assert_eq!(action.state(), &State::Executed);
    let Event::ActionExecuted(event) = event else {
        panic!("not action.executed");
    };
    assert_eq!(event.data().approval_id, Some(id("appr-01")));
}

#[test]
fn denied_verdict_starts_no_action_and_emits_no_event() {
    let verdict = win(Op::DeleteRecursive, &[WIN_HOME]);
    let refused = Action::plan(id("act-01"), &verdict, meta(T0, 1));
    assert_eq!(
        refused,
        Err(TransitionError::PlanDenied(verdict.findings().to_vec()))
    );
}

#[test]
fn execution_without_the_required_approval_is_refused() {
    let verdict = approval_needed(&[BUILD]);
    let action = planned(&verdict);
    assert_eq!(
        action.authorize(verdict.plan(), &at(T1)),
        Err(TransitionError::ApprovalRequired)
    );
    let waiting = request_approval(&action);
    assert_eq!(
        waiting.authorize(verdict.plan(), &at(T1)),
        Err(TransitionError::ApprovalRequired)
    );
    // No permit, no `executed`: the command needs a permit and the state is not `Executing`.
    let stolen = foreign_permit(&verdict_allowed_for_permit());
    assert!(matches!(
        action.apply(Command::Executed { permit: stolen }, meta(T1, 2)),
        Err(TransitionError::InvalidTransition { .. })
    ));
}

fn verdict_allowed_for_permit() -> Verdict {
    allowed()
}

#[test]
fn approval_reused_after_changing_a_target_is_refused() {
    let approved_verdict = approval_needed(&[BUILD]);
    let approved_action = approved(&approved_verdict);

    // The same request with one more target: another plan, another hash.
    let changed_verdict = approval_needed(&[BUILD, "C:\\Users\\user-a\\work\\proj\\src"]);
    assert_ne!(
        changed_verdict.plan().hash(),
        approved_verdict.plan().hash()
    );

    // 1. The approved action does not authorize the changed plan.
    assert_eq!(
        approved_action.authorize(changed_verdict.plan(), &at(T1)),
        Err(TransitionError::PlanHashMismatch)
    );

    // 2. The changed plan is a new action, and the old approval does not fit it: neither
    //    carrying the old hash, nor relabelled with the new one.
    let changed_action = planned(&changed_verdict);
    assert_eq!(
        changed_action.authorize(changed_verdict.plan(), &at(T1)),
        Err(TransitionError::ApprovalRequired)
    );
    let old_grant = Command::Grant {
        approval_id: id("appr-01"),
        plan_hash: approved_action.plan_hash().clone(),
        via: Via::LocalUi,
    };
    assert!(matches!(
        changed_action.apply(old_grant.clone(), meta(T1, 2)),
        Err(TransitionError::InvalidTransition { .. })
    ));
    let changed_waiting = changed_action
        .apply(
            Command::RequestApproval {
                approval_id: id("appr-02"),
                summary: ShortText::new("Apagar build e src").unwrap(),
                expires_at: at(EXPIRES),
            },
            meta(T0, 1),
        )
        .unwrap()
        .0;
    assert_eq!(
        changed_waiting.apply(old_grant, meta(T1, 2)),
        Err(TransitionError::PlanHashMismatch)
    );
    let relabelled = Command::Grant {
        approval_id: id("appr-01"),
        plan_hash: changed_waiting.plan_hash().clone(),
        via: Via::LocalUi,
    };
    assert_eq!(
        changed_waiting.apply(relabelled, meta(T1, 2)),
        Err(TransitionError::ApprovalMismatch)
    );

    // 3. The permit of the approved plan does not close the changed action.
    let (_, old_permit) = approved_action
        .authorize(approved_verdict.plan(), &at(T1))
        .unwrap();
    let changed_approved = changed_waiting
        .apply(
            Command::Grant {
                approval_id: id("appr-02"),
                plan_hash: changed_waiting.plan_hash().clone(),
                via: Via::Tray,
            },
            meta(T1, 2),
        )
        .unwrap()
        .0;
    let (changed_executing, _) = changed_approved
        .authorize(changed_verdict.plan(), &at(T1))
        .unwrap();
    assert_eq!(
        changed_executing.apply(Command::Executed { permit: old_permit }, meta(T1, 3)),
        Err(TransitionError::PlanHashMismatch)
    );
}

#[test]
fn changing_any_part_of_the_plan_invalidates_the_permit_check() {
    let verdict = allowed();
    let action = planned(&verdict);
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
        assert_eq!(
            action.authorize(&other, &at(T1)),
            Err(TransitionError::PlanHashMismatch),
            "{}",
            other.canonical()
        );
    }
}

#[test]
fn approval_for_a_plan_that_needs_none_is_refused() {
    let action = planned(&allowed());
    let command = Command::RequestApproval {
        approval_id: id("appr-01"),
        summary: ShortText::new("Sem necessidade").unwrap(),
        expires_at: at(EXPIRES),
    };
    assert_eq!(
        action.apply(command, meta(T0, 1)),
        Err(TransitionError::ApprovalNotRequired)
    );
}

#[test]
fn approval_cannot_be_born_expired() {
    let action = planned(&approval_needed(&[BUILD]));
    for expires_at in [T0, "2030-01-02T03:04:04.999Z"] {
        let command = Command::RequestApproval {
            approval_id: id("appr-01"),
            summary: ShortText::new("Apagar a pasta de build").unwrap(),
            expires_at: at(expires_at),
        };
        assert_eq!(
            action.apply(command, meta(T0, 1)),
            Err(TransitionError::ExpiryNotInFuture)
        );
    }
}

#[test]
fn grant_needs_the_pending_approval_the_same_hash_and_time_left() {
    let waiting = request_approval(&planned(&approval_needed(&[BUILD])));
    let grant = |approval: &str, hash: PlanHash| Command::Grant {
        approval_id: id(approval),
        plan_hash: hash,
        via: Via::RemoteDevice,
    };
    let hash = waiting.plan_hash().clone();
    assert_eq!(
        waiting.apply(grant("appr-99", hash.clone()), meta(T1, 2)),
        Err(TransitionError::ApprovalMismatch)
    );
    assert_eq!(
        waiting.apply(grant("appr-01", other_hash()), meta(T1, 2)),
        Err(TransitionError::PlanHashMismatch)
    );
    for late in [EXPIRES, AFTER] {
        assert_eq!(
            waiting.apply(grant("appr-01", hash.clone()), meta(late, 2)),
            Err(TransitionError::ApprovalLapsed)
        );
    }
    let (granted, event) = waiting.apply(grant("appr-01", hash), meta(T1, 2)).unwrap();
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
    let waiting = request_approval(&planned(&verdict));
    let deny = Command::Deny {
        approval_id: id("appr-01"),
        plan_hash: waiting.plan_hash().clone(),
        reason: Some(ShortText::new("fora de hora").unwrap()),
    };
    let wrong = Command::Deny {
        approval_id: id("appr-01"),
        plan_hash: other_hash(),
        reason: None,
    };
    assert_eq!(
        waiting.apply(wrong, meta(T1, 2)),
        Err(TransitionError::PlanHashMismatch)
    );
    let (denied, event) = waiting.apply(deny, meta(T1, 2)).unwrap();
    assert_eq!(denied.state(), &State::ApprovalDenied);
    assert!(matches!(event, Event::ApprovalDenied(_)));
    assert!(matches!(
        denied.authorize(verdict.plan(), &at(T1)),
        Err(TransitionError::InvalidTransition { .. })
    ));
}

#[test]
fn approval_expires_only_after_its_validity() {
    let verdict = approval_needed(&[BUILD]);
    let waiting = request_approval(&planned(&verdict));
    let expire = Command::Expire {
        approval_id: id("appr-01"),
        plan_hash: waiting.plan_hash().clone(),
    };
    assert_eq!(
        waiting.apply(expire.clone(), meta(T1, 2)),
        Err(TransitionError::NotYetExpired)
    );
    let (expired, event) = waiting.apply(expire, meta(EXPIRES, 2)).unwrap();
    assert_eq!(expired.state(), &State::ApprovalExpired);
    assert!(matches!(event, Event::ApprovalExpired(_)));
    assert!(matches!(
        expired.authorize(verdict.plan(), &at(AFTER)),
        Err(TransitionError::InvalidTransition { .. })
    ));
}

#[test]
fn granted_approval_does_not_outlive_its_validity() {
    let verdict = approval_needed(&[BUILD]);
    let action = approved(&verdict);
    for late in [EXPIRES, AFTER] {
        assert_eq!(
            action.authorize(verdict.plan(), &at(late)),
            Err(TransitionError::ApprovalLapsed)
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
    let (executing, permit) = planned(&verdict)
        .authorize(verdict.plan(), &at(T1))
        .unwrap();
    // A second permit is not handed out.
    assert!(matches!(
        executing.authorize(verdict.plan(), &at(T1)),
        Err(TransitionError::InvalidTransition { .. })
    ));
    // The permit of another action does not close this one.
    assert_eq!(
        executing.apply(
            Command::Executed {
                permit: foreign_permit(&verdict)
            },
            meta(T1, 2)
        ),
        Err(TransitionError::ActionMismatch)
    );
    let (executed, _) = executing
        .apply(
            Command::Executed {
                permit: permit.clone(),
            },
            meta(T1, 2),
        )
        .unwrap();
    assert!(matches!(
        executed.apply(Command::Executed { permit }, meta(T1, 3)),
        Err(TransitionError::InvalidTransition { .. })
    ));
}

#[test]
fn execution_can_fail() {
    let verdict = allowed();
    let (executing, _) = planned(&verdict)
        .authorize(verdict.plan(), &at(T1))
        .unwrap();
    let (failed, event) = executing.apply(fail(), meta(T1, 2)).unwrap();
    assert_eq!(failed.state(), &State::Failed);
    assert!(matches!(event, Event::ActionFailed(_)));
}

#[test]
fn terminal_states_accept_nothing() {
    let verdict = approval_needed(&[BUILD]);
    let waiting = request_approval(&planned(&verdict));
    let hash = waiting.plan_hash().clone();
    let denied = waiting
        .apply(
            Command::Deny {
                approval_id: id("appr-01"),
                plan_hash: hash.clone(),
                reason: None,
            },
            meta(T1, 2),
        )
        .unwrap()
        .0;
    let expired = waiting
        .apply(
            Command::Expire {
                approval_id: id("appr-01"),
                plan_hash: hash.clone(),
            },
            meta(AFTER, 2),
        )
        .unwrap()
        .0;
    let (executing, permit) = grant(&waiting).authorize(verdict.plan(), &at(T1)).unwrap();
    let executed = executing
        .apply(
            Command::Executed {
                permit: permit.clone(),
            },
            meta(T1, 2),
        )
        .unwrap()
        .0;
    let failed = executing.apply(fail(), meta(T1, 2)).unwrap().0;

    for end in [denied, expired, executed, failed] {
        assert!(end.state().is_terminal());
        let commands = [
            Command::RequestApproval {
                approval_id: id("appr-02"),
                summary: ShortText::new("De novo").unwrap(),
                expires_at: at(EXPIRES),
            },
            Command::Grant {
                approval_id: id("appr-01"),
                plan_hash: hash.clone(),
                via: Via::LocalUi,
            },
            Command::Deny {
                approval_id: id("appr-01"),
                plan_hash: hash.clone(),
                reason: None,
            },
            Command::Expire {
                approval_id: id("appr-01"),
                plan_hash: hash.clone(),
            },
            Command::Executed {
                permit: permit.clone(),
            },
            fail(),
        ];
        for command in commands {
            assert!(
                matches!(
                    end.apply(command.clone(), meta(AFTER, 9)),
                    Err(TransitionError::InvalidTransition { .. })
                ),
                "{:?} accepted {command:?}",
                end.state()
            );
        }
        assert!(matches!(
            end.authorize(verdict.plan(), &at(T1)),
            Err(TransitionError::InvalidTransition { .. })
        ));
    }
}

#[test]
fn commands_out_of_order_are_refused() {
    let verdict = approval_needed(&[BUILD]);
    let action = planned(&verdict);
    let hash = action.plan_hash().clone();
    // Decisions with no request.
    let decisions = [
        Command::Grant {
            approval_id: id("appr-01"),
            plan_hash: hash.clone(),
            via: Via::LocalUi,
        },
        Command::Deny {
            approval_id: id("appr-01"),
            plan_hash: hash.clone(),
            reason: None,
        },
        Command::Expire {
            approval_id: id("appr-01"),
            plan_hash: hash.clone(),
        },
    ];
    for command in decisions {
        assert!(matches!(
            action.apply(command, meta(T1, 2)),
            Err(TransitionError::InvalidTransition { .. })
        ));
    }
    let waiting = request_approval(&action);
    // A second request, or giving up, while one is pending.
    assert!(matches!(
        waiting.apply(
            Command::RequestApproval {
                approval_id: id("appr-02"),
                summary: ShortText::new("De novo").unwrap(),
                expires_at: at(EXPIRES),
            },
            meta(T1, 2)
        ),
        Err(TransitionError::InvalidTransition { .. })
    ));
    assert!(matches!(
        waiting.apply(fail(), meta(T1, 2)),
        Err(TransitionError::InvalidTransition { .. })
    ));
    // A second decision after the first.
    let granted = grant(&waiting);
    assert!(matches!(
        granted.apply(
            Command::Deny {
                approval_id: id("appr-01"),
                plan_hash: hash,
                reason: None,
            },
            meta(T1, 3)
        ),
        Err(TransitionError::InvalidTransition { .. })
    ));
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
    assert_eq!(
        action.authorize(measured.plan(), &at(T1)),
        Err(TransitionError::PlanHashMismatch)
    );
}
