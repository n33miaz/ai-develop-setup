//! What the executor did, or would do: the folders and the manifest it lays down first, and
//! then target by target.

use std::fmt;

use ads_core::gate::{
    Action, ActionSnapshot, Command, Event, EventMeta, ExecutionPermit, Id, JournalError,
    JournalGate, Refused,
};

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
    /// Created for this execution and removed at its end, but the system did not confirm the
    /// removal on disk: after a power loss the folder may or may not be there, empty.
    ClearedUnconfirmed { detail: String },
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
    /// After every step was done, while removing the holding folder of the execution: the
    /// report says how that removal went (`Laid::ClearedUnconfirmed`).
    Clearing,
}

/// The plan ran to the end. The permit is spent and never comes back: it is used here, once,
/// to close the action, and no method hands it, or the command that holds it, to the caller.
///
/// ```compile_fail,E0599
/// fn take(completed: ads_adapter_fs::Completed) {
///     let _ = completed.into_result();
/// }
/// ```
///
/// ```compile_fail,E0451
/// fn take(completed: ads_adapter_fs::Completed) {
///     let ads_adapter_fs::Completed { permit, .. } = completed;
/// }
/// ```
///
/// ```compile_fail,E0308
/// fn copy(started: &ads_core::gate::StartedExecution) {
///     let _: ads_core::gate::ExecutionPermit = started.permit().clone();
/// }
/// ```
///
/// What stays open to the caller is the report:
///
/// ```
/// fn read(completed: ads_adapter_fs::Completed) {
///     let ads_adapter_fs::Completed { report, .. } = completed;
///     let _ = report.untouched();
/// }
/// fn look(started: &ads_core::gate::StartedExecution) {
///     let _ = started.permit().action_id();
/// }
/// ```
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
        let (command, report) = self.into_command();
        let (action, event) = action.apply(command, meta)?;
        Ok((action, event, report))
    }

    /// Records `action.executed` for `action_id` through the journal gate. The permit goes
    /// into the gate and nowhere else: recorded or refused, it is gone, and the report comes
    /// back either way.
    pub fn record(
        self,
        gate: &mut JournalGate<'_>,
        action_id: &Id,
        meta: EventMeta,
    ) -> (Result<ActionSnapshot, JournalError>, Report) {
        let (command, report) = self.into_command();
        (gate.apply(action_id, command, meta), report)
    }

    fn into_command(self) -> (Command, Report) {
        (
            Command::Executed {
                permit: self.permit,
            },
            self.report,
        )
    }
}

/// The plan did not run, or stopped in the middle. `reason.error_code()` goes into
/// `action.failed`; `stage` and the report say exactly what was and was not changed.
#[derive(Debug, Clone)]
pub struct Failure {
    pub reason: Reason,
    pub detail: String,
    pub(crate) cause: Option<crate::reason::Cause>,
    pub stage: Stage,
    pub report: Report,
}

impl Failure {
    pub(crate) fn before_anything(stop: Stop) -> Box<Self> {
        Box::new(Self {
            reason: stop.reason,
            detail: stop.detail,
            cause: stop.cause,
            stage: Stage::Checks,
            report: Report::default(),
        })
    }
}

impl PartialEq for Failure {
    fn eq(&self, other: &Self) -> bool {
        self.reason == other.reason
            && self.detail == other.detail
            && self.stage == other.stage
            && self.report == other.report
    }
}

impl Eq for Failure {}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_deref()
            .map(|error| error as &dyn std::error::Error)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason.as_str(), self.detail)
    }
}
