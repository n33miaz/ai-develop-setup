//! The order with the log: nothing runs before `action.started` is written.
//!
//! The executor does not persist events. It hands `action.started` to a [`Journal`] the caller
//! implements and goes on only after the journal confirms. Persistence is not here; the
//! in-memory journal serves the tests and shows the duty: one `action.started` per action.

use std::fmt;

use ads_core::gate::{Event, Id};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalError(pub String);

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub trait Journal {
    /// Returns `Ok` only when `started` is durably written and is the only `action.started` of
    /// its action. Any error means the executor must not start.
    fn confirm_started(&mut self, started: &Event) -> Result<(), JournalError>;
}

/// A journal that keeps the events in memory. Refuses a second `action.started` of an action.
#[derive(Debug, Default)]
pub struct MemoryJournal {
    events: Vec<Event>,
    started: Vec<Id>,
}

impl MemoryJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
}

impl Journal for MemoryJournal {
    fn confirm_started(&mut self, started: &Event) -> Result<(), JournalError> {
        let Event::ActionStarted(envelope) = started else {
            return Err(JournalError(format!(
                "{} is not action.started",
                started.kind()
            )));
        };
        let action_id = &envelope.data().action_id;
        if self.started.contains(action_id) {
            return Err(JournalError(format!("{action_id} already started")));
        }
        self.started.push(action_id.clone());
        self.events.push(started.clone());
        Ok(())
    }
}
