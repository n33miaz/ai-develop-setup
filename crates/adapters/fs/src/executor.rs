//! The executor: the only way in is an [`ExecutionPermit`] of the gate.

use std::path::Path;

use ads_core::gate::{AbsPath, ExecutionPermit, GatePolicy};

use crate::fence::{Fence, FenceError};
use crate::limits::Limits;
use crate::prepare::prepare;
use crate::reason::{Reason, Stop};
use crate::report::{Failure, Report};
use crate::survey::{Facts, survey};

/// Runs the plan of a permit inside a fence, under a policy the caller trusts.
pub struct Executor {
    pub(crate) fence: Fence,
    pub(crate) policy: GatePolicy,
    pub(crate) limits: Limits,
}

impl Executor {
    /// `fence` is the folder nothing may leave, resolved here once. `policy` is the trusted
    /// one: the plan of every permit is judged with it again before anything happens.
    pub fn new(fence: &Path, policy: GatePolicy) -> Result<Self, FenceError> {
        Ok(Self {
            fence: Fence::new(fence)?,
            policy,
            limits: Limits::defaults(),
        })
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
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
}
