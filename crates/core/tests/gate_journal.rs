mod common;

use ads_core::gate::{
    ActionPlanRecord, Event, GateHistory, GateJournal, Id, JournalError, JournalGate,
    OperationRequest, State, Ulid,
};
use common::{POSIX_HOME, POSIX_ROOT, id, meta, plan, policy};

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
fn recovery_checks_sequences_and_reapplies_policy() {
    let original_policy = policy(POSIX_HOME, "");
    let changed_policy = policy(POSIX_HOME, "quarantine_enabled = false");
    let mut journal = planned_journal();
    let action_id = id("action-1");
    assert_eq!(
        JournalGate::new(&changed_policy, &mut journal).inspect(&action_id),
        Err(JournalError::PolicyChanged)
    );

    let first = &mut journal.history.as_mut().unwrap().events[0];
    let mut json = first.to_json();
    json["seq"] = 2.into();
    *first = Event::from_json(json).unwrap();
    assert_eq!(
        JournalGate::new(&original_policy, &mut journal).inspect(&action_id),
        Err(JournalError::Integrity)
    );
}
