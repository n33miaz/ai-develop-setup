//! The executor: the only way in is an [`ExecutionPermit`] of the gate.

use std::collections::HashSet;
use std::path::Path;

use ads_core::gate::{AbsPath, ActionPlan, Event, ExecutionPermit, GatePolicy, StartedExecution};

use crate::disk::{Disk, Probe};
use crate::fence::{Fence, FenceError};
use crate::journal::Journal;
use crate::limits::Limits;
use crate::prepare::{observe_plan, prepare};
use crate::reason::{Reason, Stop};
use crate::report::{Completed, Failure, Report};
use crate::survey::{Facts, survey};

/// Runs the plan of a permit inside a fence, under a policy the caller trusts.
pub struct Executor {
    pub(crate) fence: Fence,
    pub(crate) policy: GatePolicy,
    pub(crate) limits: Limits,
    probe: Option<Box<dyn Probe + Send>>,
    /// The actions this executor already took a permit for. A second line of defence only:
    /// it is forgotten with the executor. The record that lasts is the one of the journal.
    spent: HashSet<String>,
}

impl Executor {
    /// `fence` is the folder nothing may leave, resolved and opened here once. `policy` is
    /// the trusted one: the plan of every permit is judged with it again before anything
    /// happens.
    pub fn new(fence: &Path, policy: GatePolicy) -> Result<Self, FenceError> {
        Self::new_watched(fence, policy, || {})
    }

    /// Like [`Self::new`], calling `resolved` after the path of the fence was resolved and
    /// before the fence is opened. For tests of that instant.
    #[doc(hidden)]
    pub fn new_watched(
        fence: &Path,
        policy: GatePolicy,
        resolved: impl FnOnce(),
    ) -> Result<Self, FenceError> {
        Self::new_walked(fence, policy, resolved, |_| {})
    }

    /// Like [`Self::new_watched`], also calling `walked` with each name on the way from the
    /// root of the volume to the fence, once it is open. For tests of those instants.
    #[doc(hidden)]
    pub fn new_walked(
        fence: &Path,
        policy: GatePolicy,
        resolved: impl FnOnce(),
        mut walked: impl FnMut(&str),
    ) -> Result<Self, FenceError> {
        Ok(Self {
            fence: Fence::build(fence, resolved, &mut walked)?,
            policy,
            limits: Limits::defaults(),
            probe: None,
            spent: HashSet::new(),
        })
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Has `probe` told about every mutation, before it and right before its system call.
    pub fn with_probe(mut self, probe: Box<dyn Probe + Send>) -> Self {
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
        if !target.is_strictly_within(work_root)
            || rel.len() <= work.len()
            || !rel.starts_with(&work)
        {
            return Err(Stop::new(
                Reason::OutsideWorkRoot,
                format!("{target} is not strictly inside the work root"),
            ));
        }
        let (_, entry) = self.fence.existing(&rel)?;
        Ok(survey(&self.fence, &entry, &rel, self.limits.max_depth)?.1)
    }

    /// Inspects the requested targets before the daemon records its plan.
    pub fn observe_plan(&self, plan: &ActionPlan) -> Result<ActionPlan, Stop> {
        observe_plan(&self.fence, &self.policy, &self.limits, plan)
    }

    /// Goes through every check of an execution and says what would be done, changing
    /// nothing.
    pub fn rehearse(&self, permit: &ExecutionPermit) -> Result<Report, Box<Failure>> {
        let job = prepare(&self.fence, &self.policy, &self.limits, permit)
            .map_err(Failure::before_anything)?;
        Ok(job.report)
    }

    /// Runs the plan of `permit`. Nothing is read or changed before `journal` records that
    /// the permit of `started`, the `action.started` of this permit, is spent. Then every
    /// check of a rehearsal runs, and only then the mutations, each one reached again from
    /// the fence and made on what is open.
    ///
    /// The permit is consumed and does not come back: a success carries only the command that
    /// closes the action; a failure carries the code for `Command::Fail`, how far it went and
    /// the report of what was and was not changed. A failure in the middle stops there:
    /// nothing is undone and nothing else is touched.
    pub fn execute(
        &mut self,
        permit: ExecutionPermit,
        started: &Event,
        journal: &mut dyn Journal,
    ) -> Result<Completed, Box<Failure>> {
        if !started_by(&permit, started) {
            return Err(Failure::before_anything(Stop::new(
                Reason::StartedMismatch,
                "the event is not the action.started of this permit",
            )));
        }
        self.run(permit, started, |started| journal.consume(started))
    }

    /// Runs an authorization obtained after the gate committed its start.
    pub fn execute_recorded(
        &mut self,
        started: StartedExecution,
    ) -> Result<Completed, Box<Failure>> {
        let (permit, event) = started.into_parts();
        // Only `JournalGate::authorize` builds a `StartedExecution`, with the two together.
        debug_assert!(started_by(&permit, &event));
        self.run(permit, &event, |_| Ok(()))
    }

    fn run(
        &mut self,
        permit: ExecutionPermit,
        started: &Event,
        consume: impl FnOnce(&Event) -> Result<(), crate::JournalError>,
    ) -> Result<Completed, Box<Failure>> {
        let action_id = permit.action_id().as_str().to_owned();
        if self.spent.contains(&action_id) {
            return Err(Failure::before_anything(Stop::new(
                Reason::PermitReused,
                format!("a permit of {action_id} was already taken by this executor"),
            )));
        }
        consume(started).map_err(|error| {
            Failure::before_anything(Stop::caused_by(Reason::JournalRefused, error))
        })?;
        self.spent.insert(action_id);

        let mut job = prepare(&self.fence, &self.policy, &self.limits, &permit)
            .map_err(Failure::before_anything)?;
        if let Some(manifest) = &mut job.manifest {
            manifest.started_at = started.ts().to_string();
        }
        // The size of the manifest as it will be written, before the first change.
        let manifest = job
            .manifest_bytes(&self.limits)
            .map_err(Failure::before_anything)?;
        let floor = job.floor.clone();
        let private = job.private.clone();
        let mut disk = Disk {
            fence: &self.fence,
            floor: &floor,
            private: private.as_deref(),
            work: job.work,
            probe: self
                .probe
                .as_deref_mut()
                .map(|probe| probe as &mut (dyn Probe + 'static)),
            limits: &self.limits,
            laid: Vec::new(),
            holding: None,
        };
        let outcome = disk.run(&mut job, manifest.as_deref());
        let mut report = job.report;
        report.staging.append(&mut disk.laid);
        match outcome {
            Ok(()) => Ok(Completed { permit, report }),
            Err((stage, stop)) => Err(Box::new(Failure {
                reason: stop.reason,
                detail: stop.detail,
                cause: stop.cause,
                stage,
                report,
            })),
        }
    }
}

/// `started` is the `action.started` of `permit`: same action, same plan, same approval.
fn started_by(permit: &ExecutionPermit, started: &Event) -> bool {
    match started {
        Event::ActionStarted(envelope) => {
            let data = envelope.data();
            &data.action_id == permit.action_id()
                && &data.plan_hash == permit.plan_hash()
                && data.approval_id.as_ref() == permit.approval_id()
        }
        _ => false,
    }
}
