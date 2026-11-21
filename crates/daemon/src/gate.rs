//! Composition of the file executor with the anchored gate journal.

use std::path::Path;

use ads_adapter_fs::{Executor, Failure, Report, Stop};
use ads_core::gate::{
    AbsPath, ActionPlan, ActionSnapshot, Command, EventMeta, GatePolicy, Id, JournalError,
    JournalGate, PlanError, PlanRequest,
};
use ads_storage::EventLog;

#[derive(Debug)]
pub enum PlanActionError {
    Invalid(PlanError),
    Inspect(Stop),
    Journal(JournalError),
}

pub struct SessionGate {
    root: AbsPath,
    policy: GatePolicy,
    executor: Executor,
}

impl SessionGate {
    pub(crate) fn new(
        root: AbsPath,
        roots: Vec<AbsPath>,
        policy: GatePolicy,
        fence: &Path,
    ) -> Result<Self, String> {
        if !roots.contains(&root) {
            return Err("session working folder is no longer configured".to_owned());
        }
        let policy = policy.with_work_roots(roots);
        let executor = Executor::new(fence, policy.clone()).map_err(|error| error.to_string())?;
        Ok(Self {
            root,
            policy,
            executor,
        })
    }

    pub fn plan(
        &self,
        log: &mut EventLog,
        action_id: Id,
        mut request: PlanRequest,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, PlanActionError> {
        request.work_root = self.root.to_string();
        let plan = ActionPlan::normalize(&request).map_err(PlanActionError::Invalid)?;
        let observed = self
            .executor
            .observe_plan(&plan)
            .map_err(PlanActionError::Inspect)?;
        self.journal(log)
            .plan(action_id, &observed, meta)
            .map_err(PlanActionError::Journal)
    }

    pub fn inspect(
        &self,
        log: &mut EventLog,
        action_id: &Id,
    ) -> Result<ActionSnapshot, JournalError> {
        self.journal(log).inspect(action_id)
    }

    pub fn apply(
        &self,
        log: &mut EventLog,
        action_id: &Id,
        command: Command,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        self.journal(log).apply(action_id, command, meta)
    }

    fn journal<'a>(&'a self, log: &'a mut EventLog) -> JournalGate<'a> {
        JournalGate::new(&self.policy, log).for_work_root(&self.root)
    }

    /// Records the start before touching disk and uses it as the earliest outcome stamp.
    /// A failed outcome write preserves the operation report and leaves the start spent.
    pub fn execute(
        &mut self,
        log: &mut EventLog,
        action_id: &Id,
        start: EventMeta,
        mut finish: EventMeta,
    ) -> Result<ExecutionOutcome, JournalError> {
        let mut gate = JournalGate::new(&self.policy, log).for_work_root(&self.root);
        if finish.ts.is_before(&start.ts) {
            finish.ts = start.ts.clone();
        }
        let started = gate.authorize(action_id, start)?;
        match self.executor.execute_recorded(started) {
            Ok(completed) => {
                let (recorded, report) = completed.record(&mut gate, action_id, finish);
                Ok(ExecutionOutcome {
                    operation: Ok(report),
                    recorded,
                })
            }
            Err(failure) => {
                let command = Command::Fail {
                    error_code: failure.reason.error_code(),
                };
                Ok(ExecutionOutcome {
                    operation: Err(failure),
                    recorded: gate.apply(action_id, command, finish),
                })
            }
        }
    }
}

pub struct ExecutionOutcome {
    pub operation: Result<Report, Box<Failure>>,
    pub recorded: Result<ActionSnapshot, JournalError>,
}
