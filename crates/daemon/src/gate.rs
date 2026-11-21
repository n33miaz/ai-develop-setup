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
        if !roots
            .iter()
            .any(|path| root.is_within(path) && path.is_within(&root))
        {
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
        JournalGate::new(&self.policy, log)
    }

    pub fn execute(
        &mut self,
        log: &mut EventLog,
        action_id: &Id,
        start: EventMeta,
        finish: EventMeta,
    ) -> Result<ExecutionOutcome, JournalError> {
        execute(
            log,
            &self.policy,
            &mut self.executor,
            action_id,
            start,
            finish,
        )
    }
}

pub struct ExecutionOutcome {
    pub operation: Result<Report, Box<Failure>>,
    pub recorded: Result<ActionSnapshot, JournalError>,
}

/// Starts an action after the caller completed the required rehearsal and decision.
/// A refused start leaves the executor untouched. An outcome write failure preserves
/// the operation report and leaves the durable start spent.
pub fn execute(
    log: &mut EventLog,
    policy: &GatePolicy,
    executor: &mut Executor,
    action_id: &Id,
    start: EventMeta,
    finish: EventMeta,
) -> Result<ExecutionOutcome, JournalError> {
    let mut gate = JournalGate::new(policy, log);
    let started = gate.authorize(action_id, start)?;
    match executor.execute_recorded(started) {
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
