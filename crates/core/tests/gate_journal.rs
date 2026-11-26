mod common;

use ads_core::gate::{
    ActionPlanRecord, Command, ErrorCode, Event, GateHistory, GateJournal, Id, JournalError,
    JournalGate, OperationRequest, ShortText, State, Ulid, Via,
};
use common::{POSIX_HOME, POSIX_ROOT, at, id, meta, plan, policy, sha256_of};

const NOW: &str = "2030-01-02T03:04:05Z";

#[derive(Default)]
struct MemoryJournal {
    history: Option<GateHistory>,
    append_error: Option<JournalError>,
}

impl GateJournal for MemoryJournal {
    fn load(&mut self, _: &Id) -> Result<GateHistory, JournalError> {
        let history = self.history.as_ref().ok_or(JournalError::Missing)?;
        Ok(GateHistory {
            plan: history.plan.clone(),
            events: history.events.clone(),
        })
    }

    fn create(&mut self, plan: &ActionPlanRecord, first: &Event) -> Result<(), JournalError> {
        if self.history.is_some() {
            return Err(JournalError::Conflict);
        }
        self.history = Some(GateHistory {
            plan: plan.clone(),
            events: vec![first.clone()],
        });
        Ok(())
    }

    fn append(
        &mut self,
        _: &Id,
        expected_events: usize,
        event: &Event,
    ) -> Result<(), JournalError> {
        if let Some(error) = self.append_error {
            return Err(error);
        }
        let history = self.history.as_mut().ok_or(JournalError::Missing)?;
        assert_eq!(history.events.len(), expected_events);
        history.events.push(event.clone());
        Ok(())
    }
}

fn planned_journal() -> MemoryJournal {
    plan_journal(&policy(POSIX_HOME, ""))
}

fn plan_journal(policy: &ads_core::gate::GatePolicy) -> MemoryJournal {
    let mut journal = MemoryJournal::default();
    let plan = plan(
        OperationRequest::Delete,
        POSIX_ROOT,
        &["/home/user-a/work/proj/build"],
    );
    JournalGate::new(policy, &mut journal)
        .plan(id("action-1"), &plan, meta(NOW, 1))
        .unwrap();
    journal
}

/// The same in-memory journal stands in for a restart here; a real restart over the durable
/// log is `recovery_after_start_does_not_issue_a_second_permit` in `ads-storage`.
#[test]
fn a_permit_requires_confirmed_append_and_is_never_restored() {
    let mut journal = planned_journal();
    let policy = policy(POSIX_HOME, "");
    let action_id = id("action-1");
    let mut start = meta(NOW, 2);
    start.id = Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAC").unwrap();

    journal.append_error = Some(JournalError::Unavailable);
    assert_eq!(
        JournalGate::new(&policy, &mut journal).authorize(&action_id, start.clone()),
        Err(JournalError::Unavailable)
    );
    journal.append_error = None;
    {
        let mut gate = JournalGate::new(&policy, &mut journal);
        let permit = gate.authorize(&action_id, start.clone()).unwrap();
        assert_eq!(permit.permit().action_id(), &action_id);
    }

    let mut restarted = JournalGate::new(&policy, &mut journal);
    assert_eq!(
        restarted.inspect(&action_id).unwrap().state,
        State::Executing { approval_id: None }
    );
    assert_eq!(
        restarted.authorize(&action_id, start),
        Err(JournalError::Transition)
    );
}

#[test]
fn recovery_checks_sequences() {
    let original_policy = policy(POSIX_HOME, "");
    let mut journal = planned_journal();
    let action_id = id("action-1");
    let first = &mut journal.history.as_mut().unwrap().events[0];
    let mut json = first.to_json();
    json["seq"] = 2.into();
    *first = Event::from_json(json).unwrap();
    assert_eq!(
        JournalGate::new(&original_policy, &mut journal).inspect(&action_id),
        Err(JournalError::Integrity)
    );
}

#[test]
fn journal_rejects_retroactive_start_without_appending() {
    let mut journal = planned_journal();
    let policy = policy(POSIX_HOME, "gate_clock_rewind_seconds = 5");
    let action_id = id("action-1");
    let mut start = meta("2030-01-02T03:03:59.999999Z", 2);
    start.id = Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAC").unwrap();

    let mut gate = JournalGate::new(&policy, &mut journal);
    assert_eq!(
        gate.authorize(&action_id, start),
        Err(JournalError::OutOfOrder)
    );
    assert_eq!(gate.inspect(&action_id).unwrap().state, State::Planned);
    assert_eq!(journal.history.as_ref().unwrap().events.len(), 1);
}

#[test]
fn started_actions_close_after_lowering_clock_tolerance() {
    let tolerant = policy(POSIX_HOME, "gate_clock_rewind_seconds = 5");
    let strict = policy(POSIX_HOME, "gate_clock_rewind_seconds = 0");
    let action_id = id("action-1");
    for succeeded in [false, true] {
        let mut journal = plan_journal(&tolerant);
        let mut start = event_meta(2, 'C');
        start.ts = at("2030-01-02T03:04:00Z");
        let (permit, _) = JournalGate::new(&tolerant, &mut journal)
            .authorize(&action_id, start)
            .unwrap()
            .into_parts();

        let mut gate = JournalGate::new(&strict, &mut journal);
        let snapshot = gate.inspect(&action_id).unwrap();
        assert_eq!(snapshot.state, State::Executing { approval_id: None });
        assert_eq!(
            snapshot.policy_warning,
            Some(JournalError::HistoryOutOfOrder)
        );

        let error_code = ErrorCode::new("E_INTERRUPTED").unwrap();
        let mut too_early = event_meta(3, 'D');
        too_early.ts = at("2030-01-02T03:03:59Z");
        assert_eq!(
            gate.apply(
                &action_id,
                Command::Fail {
                    error_code: error_code.clone()
                },
                too_early
            ),
            Err(JournalError::OutOfOrder)
        );

        let (command, expected) = if succeeded {
            (Command::Executed { permit }, State::Executed)
        } else {
            (Command::Fail { error_code }, State::Failed)
        };
        let mut finish = event_meta(3, 'D');
        finish.ts = at("2030-01-02T03:04:00Z");
        assert_eq!(
            gate.apply(&action_id, command, finish).unwrap().state,
            expected
        );
        assert_eq!(gate.inspect(&action_id).unwrap().state, expected);
        assert_eq!(journal.history.as_ref().unwrap().events.len(), 3);
    }
}

#[test]
fn pending_approval_closes_after_lowering_clock_tolerance() {
    let tolerant = policy(
        POSIX_HOME,
        "approval_item_limit = 1\ngate_clock_rewind_seconds = 5",
    );
    let strict = policy(POSIX_HOME, "approval_item_limit = 1");
    let action_id = id("action-1");
    let changed = policy(
        POSIX_HOME,
        "approval_item_limit = 1\nquarantine_enabled = false",
    );
    for deny in [false, true] {
        let (
            mut journal,
            Command::Grant {
                approval_id,
                plan_hash,
                ..
            },
        ) = pending_journal(&tolerant, rewound_meta(1, 'C'))
        else {
            unreachable!();
        };
        let warning = Some(JournalError::HistoryOutOfOrder);
        assert_eq!(
            JournalGate::new(&changed, &mut journal)
                .inspect(&action_id)
                .unwrap()
                .policy_warning,
            Some(JournalError::PolicyChanged)
        );
        let mut gate = JournalGate::new(&strict, &mut journal);
        assert_eq!(gate.inspect(&action_id).unwrap().policy_warning, warning);

        let (command, expected) = if deny {
            (
                Command::Deny {
                    action_id: action_id.clone(),
                    approval_id,
                    plan_hash,
                    reason: None,
                },
                State::ApprovalDenied,
            )
        } else {
            (
                Command::Expire {
                    action_id: action_id.clone(),
                    approval_id,
                    plan_hash,
                },
                State::ApprovalExpired,
            )
        };
        let mut close = event_meta(2, 'D');
        close.ts = at("2030-01-02T03:05:05Z");
        let snapshot = gate.apply(&action_id, command, close).unwrap();
        assert_eq!(snapshot.state, expected);
        assert_eq!(snapshot.policy_warning, warning);
        assert_eq!(snapshot, gate.inspect(&action_id).unwrap());
    }
}

#[test]
fn pending_approval_with_recorded_rewind_is_denied_after_lowering_tolerance() {
    let tolerant = policy(
        POSIX_HOME,
        "approval_item_limit = 1\ngate_clock_rewind_seconds = 5",
    );
    let strict = policy(POSIX_HOME, "approval_item_limit = 1");
    let action_id = id("action-1");
    let (mut journal, grant) = pending_journal(&tolerant, rewound_meta(1, 'C'));
    let mut gate = JournalGate::new(&strict, &mut journal);
    assert_eq!(
        gate.apply(&action_id, grant, event_meta(2, 'D')),
        Err(JournalError::HistoryOutOfOrder)
    );
    assert_eq!(
        gate.inspect(&action_id).unwrap().state,
        State::ApprovalDenied
    );
    let events = &journal.history.as_ref().unwrap().events;
    let [_, _, Event::ApprovalDenied(denial)] = events.as_slice() else {
        panic!("expected a recorded approval denial");
    };
    assert_eq!(
        denial.data().reason,
        Some(ShortText::new("E_CLOCK_POLICY_CHANGED").unwrap())
    );
}

#[test]
fn approved_action_with_recorded_rewind_fails_after_lowering_tolerance() {
    let tolerant = policy(
        POSIX_HOME,
        "approval_item_limit = 1\ngate_clock_rewind_seconds = 5",
    );
    let strict = policy(POSIX_HOME, "approval_item_limit = 1");
    let action_id = id("action-1");
    let (mut journal, grant) = pending_journal(&tolerant, event_meta(1, 'C'));
    JournalGate::new(&tolerant, &mut journal)
        .apply(&action_id, grant, rewound_meta(2, 'D'))
        .unwrap();

    let mut gate = JournalGate::new(&strict, &mut journal);
    assert_eq!(
        gate.authorize(&action_id, event_meta(2, 'E')),
        Err(JournalError::HistoryOutOfOrder)
    );
    assert_eq!(gate.inspect(&action_id).unwrap().state, State::Failed);
    let events = &journal.history.as_ref().unwrap().events;
    let [_, _, _, Event::ActionFailed(failure)] = events.as_slice() else {
        panic!("expected a recorded action failure");
    };
    assert_eq!(
        failure.data().error_code,
        ErrorCode::new("E_CLOCK_POLICY_CHANGED").unwrap()
    );
}

#[test]
fn gate_clock_tolerance_rejects_queue_key_and_values_above_ceiling() {
    use ads_core::gate::{GatePolicy, HostFacts, PolicyError};

    let host = HostFacts {
        user_home: POSIX_HOME,
    };
    assert!(GatePolicy::from_toml_str("clock_rewind_seconds = 5", &host).is_err());
    assert_eq!(
        policy(POSIX_HOME, "gate_clock_rewind_seconds = 60").gate_clock_rewind_seconds(),
        60
    );
    assert!(matches!(
        GatePolicy::from_toml_str("gate_clock_rewind_seconds = 61", &host),
        Err(PolicyError::Invalid {
            key: "gate_clock_rewind_seconds",
            ..
        })
    ));
}

#[test]
fn classification_budget_refusal_leaves_the_journal_empty() {
    let plan = plan(
        OperationRequest::Delete,
        POSIX_ROOT,
        &["/home/user-a/work/proj/build"],
    );
    let mut journal = MemoryJournal::default();
    let restricted = policy(POSIX_HOME, "classification_match_steps = 0");
    assert_eq!(
        JournalGate::new(&restricted, &mut journal).plan(id("action-1"), &plan, meta(NOW, 1)),
        Err(JournalError::ClassificationBudgetExceeded(
            ads_core::gate::ClassificationBudgetExceeded,
        ))
    );
    assert!(journal.history.is_none());

    let defaults = policy(POSIX_HOME, "");
    JournalGate::new(&defaults, &mut journal)
        .plan(id("action-1"), &plan, meta(NOW, 1))
        .unwrap();
}

#[test]
fn reopening_under_a_zero_classification_budget_starts_nothing() {
    let mut journal = planned_journal();
    let action_id = id("action-1");
    let exceeded =
        JournalError::ClassificationBudgetExceeded(ads_core::gate::ClassificationBudgetExceeded);
    let restricted = policy(POSIX_HOME, "classification_match_steps = 0");
    let mut gate = JournalGate::new(&restricted, &mut journal);
    let snapshot = gate.inspect(&action_id).unwrap();
    assert_eq!(snapshot.state, State::Planned);
    assert_eq!(snapshot.policy_warning, Some(exceeded));
    assert_eq!(
        gate.authorize(&action_id, event_meta(2, 'C')),
        Err(exceeded)
    );
    assert_eq!(journal.history.as_ref().unwrap().events.len(), 1);

    let defaults = policy(POSIX_HOME, "");
    JournalGate::new(&defaults, &mut journal)
        .authorize(&action_id, event_meta(2, 'C'))
        .unwrap();
}

fn event_meta(seq: u64, suffix: char) -> ads_core::gate::EventMeta {
    let mut event = meta(NOW, seq);
    event.id = Ulid::new(format!("01ARZ3NDEKTSV4RRFFQ69G5FA{suffix}")).unwrap();
    event
}

#[test]
fn changed_policy_keeps_the_recorded_verdict_readable() {
    let mut journal = planned_journal();
    let action_id = id("action-1");
    let original = policy(POSIX_HOME, "");
    let expected = JournalGate::new(&original, &mut journal)
        .inspect(&action_id)
        .unwrap();
    let changed = policy(POSIX_HOME, "quarantine_enabled = false");
    let snapshot = JournalGate::new(&changed, &mut journal)
        .inspect(&action_id)
        .unwrap();
    assert_eq!(snapshot.state, expected.state);
    assert_eq!(snapshot.risk, expected.risk);
    assert_eq!(snapshot.plan_hash, expected.plan_hash);
    assert_eq!(snapshot.policy_warning, Some(JournalError::PolicyChanged));
}

#[test]
fn started_actions_finish_under_a_changed_policy() {
    let original = policy(POSIX_HOME, "");
    let changed = policy(POSIX_HOME, "quarantine_enabled = false");
    let action_id = id("action-1");
    for succeeded in [false, true] {
        let mut journal = planned_journal();
        let (permit, _) = JournalGate::new(&original, &mut journal)
            .authorize(&action_id, event_meta(2, 'C'))
            .unwrap()
            .into_parts();
        let (command, expected) = if succeeded {
            (Command::Executed { permit }, State::Executed)
        } else {
            (
                Command::Fail {
                    error_code: ErrorCode::new("E_INTERRUPTED").unwrap(),
                },
                State::Failed,
            )
        };
        let mut gate = JournalGate::new(&changed, &mut journal);
        assert_eq!(
            gate.apply(&action_id, command, event_meta(3, 'D'))
                .unwrap()
                .state,
            expected
        );
        assert_eq!(gate.inspect(&action_id).unwrap().state, expected);
    }
}

fn request_approval() -> Command {
    Command::RequestApproval {
        approval_id: id("approval-1"),
        summary: ShortText::new("Remove build output").unwrap(),
        expires_at: at("2030-01-02T03:05:05Z"),
    }
}

fn rewound_meta(seq: u64, suffix: char) -> ads_core::gate::EventMeta {
    let mut event = event_meta(seq, suffix);
    event.ts = at("2030-01-02T03:04:00Z");
    event
}

fn pending_journal(
    original: &ads_core::gate::GatePolicy,
    requested: ads_core::gate::EventMeta,
) -> (MemoryJournal, Command) {
    let action_id = id("action-1");
    let mut journal = plan_journal(original);
    let mut gate = JournalGate::new(original, &mut journal);
    let snapshot = gate
        .apply(&action_id, request_approval(), requested)
        .unwrap();
    let grant = Command::Grant {
        action_id,
        approval_id: id("approval-1"),
        plan_hash: snapshot.plan_hash,
        via: Via::LocalUi,
    };
    (journal, grant)
}

#[test]
fn changed_policy_refuses_to_request_approval() {
    let original = policy(POSIX_HOME, "approval_item_limit = 1");
    let changed = policy(
        POSIX_HOME,
        "approval_item_limit = 1\nquarantine_enabled = false",
    );
    let action_id = id("action-1");
    let mut journal = plan_journal(&original);

    let mut gate = JournalGate::new(&changed, &mut journal);
    assert_eq!(
        gate.apply(&action_id, request_approval(), event_meta(1, 'C')),
        Err(JournalError::PolicyChanged)
    );
    assert_eq!(gate.inspect(&action_id).unwrap().state, State::Planned);
    assert_eq!(journal.history.as_ref().unwrap().events.len(), 1);
    assert!(
        JournalGate::new(&original, &mut journal)
            .apply(&action_id, request_approval(), event_meta(1, 'C'))
            .is_ok()
    );
}

#[test]
fn recorded_target_that_cannot_be_normalized_has_its_own_warning() {
    use ads_core::gate::{PathText, PatternText, PlanHash};

    let original = policy(POSIX_HOME, "");
    let action_id = id("action-1");
    let mut journal = planned_journal();
    let history = journal.history.as_mut().unwrap();
    let mut record = history.plan.fields().clone();
    let target = &mut record.targets[0];
    let old_text = target.path.as_str();
    let path = format!("{POSIX_ROOT}/{}", "a".repeat(4095 - POSIX_ROOT.len()));
    target.pattern = Some(PatternText::new("*").unwrap());
    let new_text = format!("{path}/*");
    let canonical = history.plan.canonical().replace(
        &format!("target:0:{}:{old_text}", old_text.len()),
        &format!("target:0:{}:{new_text}", new_text.len()),
    );
    target.path = PathText::new(path).unwrap();
    record.plan_hash = PlanHash::new(sha256_of(&canonical)).unwrap();
    let mut first = history.events[0].to_json();
    first["data"]["plan_hash"] = record.plan_hash.as_str().into();
    history.events[0] = Event::from_json(first).unwrap();
    history.plan = ActionPlanRecord::try_from(record).unwrap();

    let mut gate = JournalGate::new(&original, &mut journal);
    let snapshot = gate.inspect(&action_id).unwrap();
    assert_eq!(snapshot.state, State::Planned);
    assert_eq!(
        snapshot.policy_warning,
        Some(JournalError::PlanNormalizationFailed)
    );
    assert_eq!(
        gate.authorize(&action_id, event_meta(2, 'C')),
        Err(JournalError::PlanNormalizationFailed)
    );
    assert_eq!(journal.history.as_ref().unwrap().events.len(), 1);
}

#[test]
fn changed_policy_refuses_and_denies_a_pending_approval() {
    let original = policy(POSIX_HOME, "approval_item_limit = 1");
    let changed = policy(
        POSIX_HOME,
        "approval_item_limit = 1\nquarantine_enabled = false",
    );
    let action_id = id("action-1");
    let (mut journal, grant) = pending_journal(&original, event_meta(1, 'C'));

    let mut gate = JournalGate::new(&changed, &mut journal);
    assert_eq!(
        gate.apply(&action_id, grant, event_meta(2, 'D')),
        Err(JournalError::PolicyChanged)
    );
    assert_eq!(
        gate.inspect(&action_id).unwrap().state,
        State::ApprovalDenied
    );
    let last = journal.history.as_ref().unwrap().events.last().unwrap();
    assert_eq!(last.to_json()["data"]["reason"], "E_POLICY_CHANGED");
}

#[test]
fn changed_policy_refuses_to_start_a_planned_action() {
    let original = policy(POSIX_HOME, "");
    let action_id = id("action-1");
    for changed in [
        policy(POSIX_HOME, "approval_item_limit = 1"),
        original.clone().with_product_paths(vec![
            ads_core::gate::AbsPath::parse("/home/user-a/work/proj/build").unwrap(),
        ]),
    ] {
        let mut journal = planned_journal();
        let mut gate = JournalGate::new(&changed, &mut journal);
        assert_eq!(
            gate.authorize(&action_id, event_meta(2, 'C')),
            Err(JournalError::PolicyChanged)
        );
        assert_eq!(gate.inspect(&action_id).unwrap().state, State::Failed);
        let events = &journal.history.as_ref().unwrap().events;
        let [Event::ActionPlanned(_), Event::ActionFailed(failure)] = events.as_slice() else {
            panic!("expected only planning and failure events");
        };
        assert_eq!(
            failure.data().error_code,
            ErrorCode::new("E_POLICY_CHANGED").unwrap()
        );
        assert_eq!(
            JournalGate::new(&original, &mut journal).authorize(&action_id, event_meta(3, 'D')),
            Err(JournalError::Transition)
        );
    }
}

#[test]
fn changed_policy_refuses_and_closes_an_approved_action() {
    let original = policy(POSIX_HOME, "approval_item_limit = 1");
    let changed = policy(
        POSIX_HOME,
        "approval_item_limit = 1\nquarantine_enabled = false",
    );
    let action_id = id("action-1");
    let (mut journal, grant) = pending_journal(&original, event_meta(1, 'C'));
    JournalGate::new(&original, &mut journal)
        .apply(&action_id, grant, event_meta(2, 'D'))
        .unwrap();

    let mut gate = JournalGate::new(&changed, &mut journal);
    assert_eq!(
        gate.authorize(&action_id, event_meta(2, 'E')),
        Err(JournalError::PolicyChanged)
    );
    assert_eq!(gate.inspect(&action_id).unwrap().state, State::Failed);
    let last = journal.history.as_ref().unwrap().events.last().unwrap();
    assert_eq!(last.to_json()["data"]["error_code"], "E_POLICY_CHANGED");
    let mut gate = JournalGate::new(&original, &mut journal);
    assert_eq!(
        gate.authorize(&action_id, event_meta(3, 'F')),
        Err(JournalError::Transition)
    );
}
