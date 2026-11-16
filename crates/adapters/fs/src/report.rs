//! What the executor did, or would do: the folders and the manifest it lays down first, and
//! then target by target.

use std::fmt;

use ads_core::gate::{Action, Command, Event, EventMeta, ExecutionPermit, Refused};

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
    /// Made, and the folders involved are confirmed on disk.
    Done,
    /// Listed in the manifest, not in quarantine: there was nothing to move.
    Absent,
    /// The step stopped before its target was changed: it is where it was, whole.
    Failed { reason: Reason, detail: String },
    /// A definitive delete stopped after its target was moved to its holding folder (`held`
    /// of the step): what is left of it is there. `removed` counts the entries seen to go;
    /// `confirmed`, those whose folder was confirmed on disk afterwards.
    Held {
        reason: Reason,
        detail: String,
        removed: u64,
        confirmed: u64,
    },
    /// The change was made, but the system did not confirm it is on disk: after a power
    /// loss it may or may not be there.
    Unconfirmed { detail: String },
    /// The system made the move, but the check after it did not confirm what is under the
    /// destination name. The target left its place; where it is now is not promised.
    Unverified { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The path the step is about, as the plan (or the manifest) writes it.
    pub target: String,
    pub effect: Effect,
    pub facts: Facts,
    /// Where a tree is held while it is removed for good: it is moved there first, whole,
    /// and removed there. `None` for every other step. Only a place the step tried: what is
    /// there is promised by `Held` alone, never by `Unverified`.
    pub held: Option<String>,
    pub status: Status,
}

/// What the executor lays down to hold a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scaffold {
    Folder,
    Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Laid {
    /// Not touched.
    Pending,
    /// The folder was already there: nothing was changed.
    AlreadyThere,
    /// Created, and confirmed on disk.
    Made,
    /// Created for this execution and removed at its end.
    Cleared,
    /// Not created.
    Failed { reason: Reason, detail: String },
    /// Created, and it stopped there: a file whose content was not written to the end, or
    /// something the system did not confirm is on disk. It exists; it is not to be trusted.
    Unconfirmed { detail: String },
}

/// One change of infrastructure: a folder or the manifest of a batch. These are not targets,
/// but they are changes on disk, and stay there when an execution stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    pub path: String,
    pub what: Scaffold,
    pub status: Laid,
}

/// The infrastructure in the order it is laid, then one step per target, in the order of
/// execution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub staging: Vec<Staged>,
    pub steps: Vec<Step>,
}

impl Report {
    pub fn done(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|step| step.status == Status::Done)
    }

    pub fn not_done(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|step| step.status != Status::Done)
    }

    /// Nothing on disk was changed: no folder or manifest was created and no target was
    /// moved or removed, not even in part.
    pub fn untouched(&self) -> bool {
        let laid = self.staging.iter().any(|staged| {
            !matches!(
                staged.status,
                Laid::Pending | Laid::AlreadyThere | Laid::Failed { .. }
            )
        });
        let stepped = self.steps.iter().any(|step| match &step.status {
            Status::Pending | Status::Absent | Status::Failed { .. } => false,
            Status::Done
            | Status::Held { .. }
            | Status::Unconfirmed { .. }
            | Status::Unverified { .. } => true,
        });
        !laid && !stepped
    }
}

/// How far an execution went before it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// During the checks: nothing on disk was changed.
    Checks,
    /// While laying the folders and the manifest of the batch: no target was touched, but
    /// what the report lists as made is on disk.
    Staging,
    /// At a target: the report says which, and what was done before it.
    Steps,
}

/// The plan ran to the end. The permit is spent and never comes back: it is used here, once,
/// to close the action.
#[derive(Debug)]
pub struct Completed {
    pub(crate) permit: ExecutionPermit,
    pub report: Report,
}

impl Completed {
    /// Closes `action` in the gate (`Command::Executed`), yielding `action.executed`. The
    /// permit goes into the gate and nowhere else: accepted or refused, it is gone.
    pub fn close(
        self,
        action: Action,
        meta: EventMeta,
    ) -> Result<(Action, Event, Report), Box<Refused>> {
        let command = Command::Executed {
            permit: self.permit,
        };
        let (action, event) = action.apply(command, meta)?;
        Ok((action, event, self.report))
    }
}

/// The plan did not run, or stopped in the middle. `reason.error_code()` goes into
/// `action.failed`; `stage` and the report say exactly what was and was not changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub reason: Reason,
    pub detail: String,
    pub stage: Stage,
    pub report: Report,
}

impl Failure {
    pub(crate) fn before_anything(stop: Stop) -> Box<Self> {
        Box::new(Self {
            reason: stop.reason,
            detail: stop.detail,
            stage: Stage::Checks,
            report: Report::default(),
        })
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason.as_str(), self.detail)
    }
}
