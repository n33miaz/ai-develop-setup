//! Gate for destructive file system actions. Nothing here touches the file system: the gate
//! takes a plan as data and answers whether, and as what, it may run.
//!
//! The types only fit together one way: a [`PlanRequest`] becomes an [`ActionPlan`] only by
//! normalization, a [`Verdict`] comes only from [`classify`], an [`Action`] only from a verdict
//! that is not "denied", and an [`ExecutionPermit`] only from an action whose plan hash and
//! approval match.

mod classify;
mod event;
mod machine;
mod path;
mod plan;
mod policy;

pub use classify::{Decision, Finding, Rule, Verdict, classify};
pub use event::{
    ActionExecuted, ActionFailed, ActionPlanned, Actor, ActorKind, ApprovalDenied, ApprovalExpired,
    ApprovalGranted, ApprovalRequested, ContractError, DataClass, Envelope, ErrorCode, Event,
    EventData, EventMeta, Id, PlanHash, Risk, ShortText, Timestamp, Ulid, Verb, Via,
};
pub use machine::{Action, Command, ExecutionPermit, State, TransitionError};
pub use path::{AbsPath, PathError, PathStyle, TargetPath};
pub use plan::{
    ActionPlan, Estimate, Operation, OperationRequest, PlanError, PlanRequest, Target,
    TargetRequest,
};
pub use policy::{GatePolicy, HostFacts, PolicyError, Protection};
