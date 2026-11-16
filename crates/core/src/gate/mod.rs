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
mod machine;
mod path;
mod plan;
mod policy;

pub use classify::{Finding, Rule, Verdict, classify};
pub use event::{
    ActionDenied, ActionExecuted, ActionFailed, ActionPlanFields, ActionPlanRecord, ActionPlanned,
    ActionStarted, Actor, ActorKind, ApprovalDenied, ApprovalDeniedV1, ApprovalExpired,
    ApprovalExpiredV1, ApprovalFields, ApprovalGranted, ApprovalGrantedV1, ApprovalRecord,
    ApprovalRequested, ApprovalRequestedV1, ApprovalStatus, ContractError, DataClass, Decision,
    Envelope, ErrorCode, Event, EventData, EventMeta, Id, PathText, PatternText, PlanEstimate,
    PlanHash, PlanOperation, PlanReason, PlanTarget, PlanVerdict, Risk, RuleCode, RuleList,
    ShortText, SubjectText, Timestamp, Ulid, Verb, Via,
};
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
