//! Gate for destructive file system actions. Nothing here touches the file system: the gate
//! takes a plan as data and answers whether, and as what, it may run.
//!
//! The types only fit together one way: a [`PlanRequest`] becomes an [`ActionPlan`] only by
//! normalization, a [`Verdict`] comes only from [`classify`], an [`Action`] only from a verdict
//! (a denied one is born in its final state), and an [`ExecutionPermit`] only from an action
//! whose plan hash and approval match. Every state is rebuilt from the events alone
//! ([`replay`]).

mod classify;
mod event;
mod journal;
mod machine;
mod path;
mod plan;
mod policy;

pub use classify::{Finding, Rule, Verdict, classify, may_name};
pub use event::{
    ActionDenied, ActionExecuted, ActionFailed, ActionPlanFields, ActionPlanRecord,
    ActionPlanRecordV1, ActionPlanned, ActionStarted, Actor, ActorKind, ApprovalDenied,
    ApprovalDeniedV1, ApprovalExpired, ApprovalExpiredV1, ApprovalFields, ApprovalGranted,
    ApprovalGrantedV1, ApprovalRecord, ApprovalRequested, ApprovalRequestedV1, ApprovalStatus,
    ContractError, DataClass, Decision, ESTIMATE_MAX, Envelope, ErrorCode, Event, EventData,
    EventMeta, Id, PathText, PatternText, PlanEstimate, PlanHash, PlanOperation, PlanReason,
    PlanTarget, PlanVerdict, Risk, RuleCode, RuleList, ShortText, SubjectText, Timestamp, Ulid,
    Verb, Via,
};
pub use journal::{GateHistory, GateJournal, JournalError, JournalGate, StartedExecution};
pub use machine::{
    Action, ActionSnapshot, Command, ExecutionPermit, Refused, ReplayError, State, TransitionError,
    replay,
};
pub use path::{AbsPath, PathError, PathStyle, TargetPath};
pub use plan::{
    ActionPlan, Estimate, Operation, OperationRequest, PlanError, PlanRequest, Target,
    TargetRequest,
};
pub use policy::{GatePolicy, HostFacts, PolicyError, Protection};

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn gate_failures_are_standard_errors() {
        fn assert_error<T: Error>() {}

        assert_error::<ContractError>();
        assert_error::<PathError>();
        assert_error::<PlanError>();
        assert_error::<PolicyError>();
        assert_error::<Refused>();
        assert_error::<ReplayError>();
        assert_error::<TransitionError>();
    }

    #[test]
    fn gate_errors_expose_their_typed_causes() {
        let path = AbsPath::parse("relative").unwrap_err();
        for plan in [
            PlanError::WorkRoot(path),
            PlanError::Destination(path),
            PlanError::Target {
                index: 0,
                error: path,
            },
        ] {
            assert_eq!(
                plan.source().unwrap().downcast_ref::<PathError>(),
                Some(&path)
            );
        }
        let host = HostFacts {
            user_home: "/home/test",
        };
        let policy = GatePolicy::from_toml_str("allowed_roots = [1]", &host).unwrap_err();
        let cause = policy
            .source()
            .unwrap()
            .downcast_ref::<toml::de::Error>()
            .unwrap();
        assert_eq!(policy.to_string(), cause.to_string());

        let policy = GatePolicy::from_toml_str("allowed_roots = ['relative']", &host).unwrap_err();
        assert_eq!(
            policy.source().unwrap().downcast_ref::<PathError>(),
            Some(&path)
        );

        let error = TransitionError::Unrecordable(ContractError::new("plan", "invalid"));
        assert!(error.source().unwrap().is::<ContractError>());
        let error = TransitionError::Inconsistent(ReplayError::EmptyLog);
        assert_eq!(
            error.source().unwrap().downcast_ref::<ReplayError>(),
            Some(&ReplayError::EmptyLog)
        );
        assert!(TransitionError::ApprovalRequired.source().is_none());
    }
}
