//! Composition of the file executor with the anchored gate journal.

use ads_adapter_fs::{Executor, Failure, Report};
use ads_core::gate::{
    ActionSnapshot, Command, EventMeta, GatePolicy, Id, JournalError, JournalGate,
};
use ads_storage::EventLog;

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
