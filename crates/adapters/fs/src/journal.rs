//! The order with the log, and the single use of a permit.
//!
//! The executor does not persist events. It hands `action.started` to a [`Journal`] the caller
//! implements and goes on only after the journal confirms. That confirmation is what makes a
//! permit good for one execution: the executor keeps no state between runs, so the record
//! that an action was already executed has to live in the journal. There is no journal in
//! this crate on purpose: one kept in memory forgets at every restart, and serves only tests.

use std::fmt;

use ads_core::gate::Event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalError(pub String);

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub trait Journal {
    /// Records that the permit of `started` (the `action.started` of one action) is spent.
    ///
    /// Has to return `Ok` at most once per action, ever: the record is durable before the
    /// call returns, the test "was it recorded already" and the write are one atomic step,
    /// and the record is shared by every executor and survives restarts. A second call for
    /// the same action returns an error, and so does any doubt. Any error means the executor
    /// does not start.
    fn consume(&mut self, started: &Event) -> Result<(), JournalError>;
}
