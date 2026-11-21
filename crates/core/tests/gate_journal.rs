mod common;

use ads_core::gate::{
    ActionPlanRecord, Command, ErrorCode, Event, GateHistory, GateJournal, Id, JournalError,
    JournalGate, OperationRequest, ShortText, State, Ulid, Via,
};
use common::{POSIX_HOME, POSIX_ROOT, at, id, meta, plan, policy};

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
    let mut journal = MemoryJournal::default();
    let plan = plan(
        OperationRequest::Delete,
        POSIX_ROOT,
        &["/home/user-a/work/proj/build"],
    );
    let policy = policy(POSIX_HOME, "");
    JournalGate::new(&policy, &mut journal)
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
fn journal_replays_with_configured_tolerance() {
    let mut journal = planned_journal();
    let tolerant = policy(POSIX_HOME, "gate_clock_rewind_seconds = 5");
    let strict = policy(POSIX_HOME, "");
    let action_id = id("action-1");
    let mut start = meta("2030-01-02T03:04:00Z", 2);
    start.id = Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAC").unwrap();

    let mut gate = JournalGate::new(&tolerant, &mut journal);
    gate.authorize(&action_id, start).unwrap();
    assert_eq!(
        gate.inspect(&action_id).unwrap().state,
        State::Executing { approval_id: None }
    );
    assert_eq!(
        JournalGate::new(&strict, &mut journal).inspect(&action_id),
        Err(JournalError::HistoryOutOfOrder),
    );
    assert_eq!(journal.history.as_ref().unwrap().events.len(), 2);
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

fn pending_journal(original: &ads_core::gate::GatePolicy) -> (MemoryJournal, Command) {
    let action_id = id("action-1");
    let approval_id = id("approval-1");
    let plan = plan(
        OperationRequest::Delete,
        POSIX_ROOT,
        &["/home/user-a/work/proj/build"],
    );
    let mut journal = MemoryJournal::default();
    let mut gate = JournalGate::new(original, &mut journal);
    let snapshot = gate.plan(action_id.clone(), &plan, meta(NOW, 1)).unwrap();
    gate.apply(
        &action_id,
        Command::RequestApproval {
            approval_id: approval_id.clone(),
            summary: ShortText::new("Remove build output").unwrap(),
            expires_at: at("2030-01-02T03:05:05Z"),
        },
        event_meta(1, 'C'),
    )
    .unwrap();
    let grant = Command::Grant {
        action_id,
        approval_id,
        plan_hash: snapshot.plan_hash,
        via: Via::LocalUi,
    };
    (journal, grant)
}

#[test]
fn changed_policy_refuses_and_denies_a_pending_approval() {
    let original = policy(POSIX_HOME, "approval_item_limit = 1");
    let changed = policy(
        POSIX_HOME,
        "approval_item_limit = 1\nquarantine_enabled = false",
    );
    let action_id = id("action-1");
    let (mut journal, grant) = pending_journal(&original);

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
fn changed_policy_refuses_and_closes_an_approved_action() {
    let original = policy(POSIX_HOME, "approval_item_limit = 1");
    let changed = policy(
        POSIX_HOME,
        "approval_item_limit = 1\nquarantine_enabled = false",
    );
    let action_id = id("action-1");
    let (mut journal, grant) = pending_journal(&original);
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
