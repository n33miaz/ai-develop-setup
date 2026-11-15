//! The executor: the only way in is an [`ExecutionPermit`] of the gate.

use std::path::Path;

use ads_core::gate::{AbsPath, Event, ExecutionPermit, GatePolicy};

use crate::disk::{Disk, Probe};
use crate::fence::{Fence, FenceError};
use crate::journal::Journal;
use crate::limits::Limits;
use crate::prepare::prepare;
use crate::reason::{Reason, Stop};
use crate::report::{Completed, Failure, Report};
use crate::survey::{Facts, survey};

/// Runs the plan of a permit inside a fence, under a policy the caller trusts.
pub struct Executor {
    pub(crate) fence: Fence,
    pub(crate) policy: GatePolicy,
    pub(crate) limits: Limits,
    probe: Option<Box<dyn Probe>>,
}

impl Executor {
    /// `fence` is the folder nothing may leave, resolved here once. `policy` is the trusted
    /// one: the plan of every permit is judged with it again before anything happens.
    pub fn new(fence: &Path, policy: GatePolicy) -> Result<Self, FenceError> {
        Ok(Self {
            fence: Fence::new(fence)?,
            policy,
            limits: Limits::defaults(),
            probe: None,
        })
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Has `probe` told about every mutation before its last check.
    pub fn with_probe(mut self, probe: Box<dyn Probe>) -> Self {
        self.probe = Some(probe);
        self
    }

    /// The fence as a plan writes it.
    pub fn fence_text(&self) -> String {
        self.fence.text()
    }

    /// What is on disk at `target`, to fill a plan with facts instead of guesses. Read only.
    pub fn measure(&self, work_root: &AbsPath, target: &AbsPath) -> Result<Facts, Stop> {
        let work = self.fence.relative(work_root)?;
        let rel = self.fence.relative(target)?;
        if !target.is_strictly_within(work_root) || rel.len() <= work.len() {
            return Err(Stop::new(
                Reason::OutsideWorkRoot,
                format!("{target} is not strictly inside the work root"),
            ));
        }
        Ok(survey(&self.fence, &rel, self.limits.max_depth)?.1)
    }

    /// Goes through every check of an execution and says what would be done, changing
    /// nothing.
    pub fn rehearse(&self, permit: &ExecutionPermit) -> Result<Report, Box<Failure>> {
        let job = prepare(&self.fence, &self.policy, &self.limits, permit)
            .map_err(Failure::before_anything)?;
        Ok(Report { steps: job.steps })
    }

    /// Runs the plan of `permit`. Nothing is read or changed before `journal` confirms that
    /// `started`, the `action.started` of this permit, is written. Then every check of a
    /// rehearsal runs, and only then the mutations, each one checked again right before.
    ///
    /// The permit is consumed. It comes back only when the whole plan ran, to close the action
    /// with `Command::Executed`; a failure carries the code for `Command::Fail` and the report
    /// of what was and was not done. A failure in the middle stops there: nothing is undone
    /// and nothing else is touched.
    pub fn execute(
        &mut self,
        permit: ExecutionPermit,
        started: &Event,
        journal: &mut dyn Journal,
    ) -> Result<Completed, Box<Failure>> {
        let matches = match started {
            Event::ActionStarted(envelope) => {
                let data = envelope.data();
                &data.action_id == permit.action_id()
                    && &data.plan_hash == permit.plan_hash()
                    && data.approval_id.as_ref() == permit.approval_id()
            }
            _ => false,
        };
        if !matches {
            return Err(Failure::before_anything(Stop::new(
                Reason::StartedMismatch,
                "the event is not the action.started of this permit",
            )));
        }
        journal
            .confirm_started(started)
            .map_err(|error| Failure::before_anything(Stop::new(Reason::JournalRefused, error)))?;

        let mut job = prepare(&self.fence, &self.policy, &self.limits, &permit)
            .map_err(Failure::before_anything)?;
        if let Some((_, manifest)) = &mut job.manifest {
            manifest.started_at = started.ts().to_string();
        }
        let floor = job.floor.clone();
        let mut disk = Disk {
            fence: &self.fence,
            floor: &floor,
            probe: self.probe.as_deref_mut(),
            max_depth: self.limits.max_depth,
        };
        let outcome = disk.run(&mut job);
        let report = Report { steps: job.steps };
        match outcome {
            Ok(()) => Ok(Completed { permit, report }),
            Err(stop) => Err(Box::new(Failure {
                reason: stop.reason,
                detail: stop.detail,
                report,
            })),
        }
    }
}
