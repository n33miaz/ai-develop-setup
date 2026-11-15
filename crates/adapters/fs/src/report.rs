//! What the executor did, or would do, target by target.

use std::fmt;

use ads_core::gate::ExecutionPermit;

use crate::reason::{Reason, Stop};
use crate::survey::Facts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Moved into the quarantine batch, to `stored`.
    Quarantine {
        stored: String,
    },
    /// Moved back from `stored` to where it came from.
    Restore {
        stored: String,
    },
    Delete,
    DeleteRecursive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Not touched: a rehearsal, or the batch stopped before this step.
    Pending,
    Done,
    /// Listed in the manifest, not in quarantine: there was nothing to move.
    Absent,
    /// The step stopped here. `removed` counts the entries of a recursive delete that were
    /// already gone; a move either happened or did not.
    Failed {
        reason: Reason,
        detail: String,
        removed: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The path the step is about, as the plan (or the manifest) writes it.
    pub target: String,
    pub effect: Effect,
    pub facts: Facts,
    pub status: Status,
}

/// One step per target, in the order of execution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub steps: Vec<Step>,
}

impl Report {
    pub fn done(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|step| step.status == Status::Done)
    }

    pub fn not_done(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|step| step.status != Status::Done)
    }
}

/// The plan ran to the end. The permit comes back to close the action (`Command::Executed`).
#[derive(Debug)]
pub struct Completed {
    pub permit: ExecutionPermit,
    pub report: Report,
}

/// The plan did not run, or stopped in the middle. `reason.error_code()` goes into
/// `action.failed`; the report says exactly what was and was not done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub reason: Reason,
    pub detail: String,
    pub report: Report,
}

impl Failure {
    pub(crate) fn before_anything(stop: Stop) -> Box<Self> {
        Box::new(Self {
            reason: stop.reason,
            detail: stop.detail,
            report: Report::default(),
        })
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason.as_str(), self.detail)
    }
}
