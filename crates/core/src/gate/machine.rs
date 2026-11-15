//! The life of one action: planned, approved when the verdict asks for it, executed or failed.
//! Every transition is a pure function that refuses any jump and yields the contract event.

use std::fmt;

use super::classify::{Decision, Finding, Verdict};
use super::event::{
    ActionExecuted, ActionFailed, ActionPlanned, ApprovalDenied, ApprovalExpired, ApprovalGranted,
    ApprovalRequested, Envelope, ErrorCode, Event, EventMeta, Id, PlanHash, Risk, ShortText,
    Timestamp, Via,
};
use super::plan::ActionPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Planned,
    AwaitingApproval {
        approval_id: Id,
        expires_at: Timestamp,
    },
    Approved {
        approval_id: Id,
        expires_at: Timestamp,
    },
    /// An execution permit was handed out. No contract event marks this step.
    Executing {
        approval_id: Option<Id>,
    },
    ApprovalDenied,
    ApprovalExpired,
    Executed,
    Failed,
}

impl State {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ApprovalDenied | Self::ApprovalExpired | Self::Executed | Self::Failed
        )
    }
}

/// What may happen to a planned action, apart from handing out the execution permit
/// ([`Action::authorize`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    RequestApproval {
        approval_id: Id,
        summary: ShortText,
        expires_at: Timestamp,
    },
    Grant {
        approval_id: Id,
        plan_hash: PlanHash,
        via: Via,
    },
    Deny {
        approval_id: Id,
        plan_hash: PlanHash,
        reason: Option<ShortText>,
    },
    Expire {
        approval_id: Id,
        plan_hash: PlanHash,
    },
    /// The executor finished the plan of the permit.
    Executed {
        permit: ExecutionPermit,
    },
    Fail {
        error_code: ErrorCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// The verdict was "denied": the action does not exist, and no event is produced.
    PlanDenied(Vec<Finding>),
    /// The command does not fit the current state.
    InvalidTransition {
        state: State,
        command: &'static str,
    },
    /// The verdict allowed the plan as it is; there is nothing to approve.
    ApprovalNotRequired,
    /// Execution asked for a plan the verdict sent to approval, with no approval granted.
    ApprovalRequired,
    /// The approval, the permit or the plan presented is for another hash.
    PlanHashMismatch,
    /// The approval or the permit named is not the one of this action.
    ApprovalMismatch,
    ActionMismatch,
    /// The approval would be born expired.
    ExpiryNotInFuture,
    /// The validity of the approval is over: too late to grant or to execute.
    ApprovalLapsed,
    /// Expiring an approval whose validity is not over.
    NotYetExpired,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanDenied(findings) => {
                write!(f, "the plan was denied by {} rule(s)", findings.len())
            }
            Self::InvalidTransition { state, command } => {
                write!(f, "`{command}` is not accepted in state {state:?}")
            }
            Self::ApprovalNotRequired => f.write_str("the plan does not need approval"),
            Self::ApprovalRequired => f.write_str("the plan needs an approval it does not have"),
            Self::PlanHashMismatch => f.write_str("another plan hash"),
            Self::ApprovalMismatch => f.write_str("another approval"),
            Self::ActionMismatch => f.write_str("another action"),
            Self::ExpiryNotInFuture => f.write_str("the approval would be born expired"),
            Self::ApprovalLapsed => f.write_str("the validity of the approval is over"),
            Self::NotYetExpired => f.write_str("the validity of the approval is not over"),
        }
    }
}

/// The only thing an executor accepts: proof that this exact plan passed the gate. Only
/// [`Action::authorize`] builds one, and each action hands out a single permit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPermit {
    action_id: Id,
    plan_hash: PlanHash,
    approval_id: Option<Id>,
    plan: ActionPlan,
}

impl ExecutionPermit {
    pub fn action_id(&self) -> &Id {
        &self.action_id
    }

    pub fn plan_hash(&self) -> &PlanHash {
        &self.plan_hash
    }

    pub fn approval_id(&self) -> Option<&Id> {
        self.approval_id.as_ref()
    }

    /// The plan to execute, exactly as approved.
    pub fn plan(&self) -> &ActionPlan {
        &self.plan
    }
}

/// One action going through the gate. Only [`Action::plan`] builds one, from a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    action_id: Id,
    plan_hash: PlanHash,
    risk: Risk,
    needs_approval: bool,
    state: State,
}

impl Action {
    /// Starts the action from the plan of the verdict (the quarantine rewrite, when there is
    /// one). A denied verdict starts nothing.
    pub fn plan(
        action_id: Id,
        verdict: &Verdict,
        meta: EventMeta,
    ) -> Result<(Action, Event), TransitionError> {
        let needs_approval = match verdict.decision() {
            Decision::Allowed => false,
            Decision::RequiresApproval => true,
            Decision::Denied => {
                return Err(TransitionError::PlanDenied(verdict.findings().to_vec()));
            }
        };
        let action = Action {
            action_id,
            plan_hash: verdict.plan().hash(),
            risk: verdict.risk(),
            needs_approval,
            state: State::Planned,
        };
        let event = Event::ActionPlanned(Envelope::new(
            meta,
            ActionPlanned {
                action_id: action.action_id.clone(),
                verb: verdict.plan().operation().verb(),
                risk: action.risk,
                plan_hash: action.plan_hash.clone(),
                // The core never runs elevated and no rule here asks for it.
                needs_elevation: false,
            },
        ));
        Ok((action, event))
    }

    pub fn action_id(&self) -> &Id {
        &self.action_id
    }

    pub fn plan_hash(&self) -> &PlanHash {
        &self.plan_hash
    }

    pub fn risk(&self) -> Risk {
        self.risk
    }

    pub fn needs_approval(&self) -> bool {
        self.needs_approval
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    fn with(&self, state: State) -> Action {
        Action {
            state,
            ..self.clone()
        }
    }

    fn invalid(&self, command: &'static str) -> TransitionError {
        TransitionError::InvalidTransition {
            state: self.state.clone(),
            command,
        }
    }

    /// The pending approval, after checking that the command names it and this plan.
    fn pending(
        &self,
        command: &'static str,
        approval_id: &Id,
        plan_hash: &PlanHash,
    ) -> Result<&Timestamp, TransitionError> {
        let State::AwaitingApproval {
            approval_id: pending,
            expires_at,
        } = &self.state
        else {
            return Err(self.invalid(command));
        };
        if plan_hash != &self.plan_hash {
            return Err(TransitionError::PlanHashMismatch);
        }
        if approval_id != pending {
            return Err(TransitionError::ApprovalMismatch);
        }
        Ok(expires_at)
    }

    /// Hands out the permit to execute `plan` at instant `now`, and moves to `Executing`.
    /// Refused when the plan is not the one planned, when the verdict asked for an approval
    /// that was not granted, or when the granted approval is no longer valid.
    pub fn authorize(
        &self,
        plan: &ActionPlan,
        now: &Timestamp,
    ) -> Result<(Action, ExecutionPermit), TransitionError> {
        let approval_id = match &self.state {
            State::Planned if !self.needs_approval => None,
            State::Planned | State::AwaitingApproval { .. } => {
                return Err(TransitionError::ApprovalRequired);
            }
            State::Approved {
                approval_id,
                expires_at,
            } => {
                if !now.is_before(expires_at) {
                    return Err(TransitionError::ApprovalLapsed);
                }
                Some(approval_id.clone())
            }
            _ => return Err(self.invalid("authorize")),
        };
        let plan_hash = plan.hash();
        if plan_hash != self.plan_hash {
            return Err(TransitionError::PlanHashMismatch);
        }
        let permit = ExecutionPermit {
            action_id: self.action_id.clone(),
            plan_hash,
            approval_id: approval_id.clone(),
            plan: plan.clone(),
        };
        Ok((self.with(State::Executing { approval_id }), permit))
    }

    /// Applies one command. `meta.ts` is the instant of the transition; `meta.seq` counts in
    /// the stream of the event produced (`approval/<id>` for approvals, `action/<id>` for the
    /// rest).
    pub fn apply(
        &self,
        command: Command,
        meta: EventMeta,
    ) -> Result<(Action, Event), TransitionError> {
        match command {
            Command::RequestApproval {
                approval_id,
                summary,
                expires_at,
            } => {
                if self.state != State::Planned {
                    return Err(self.invalid("request_approval"));
                }
                if !self.needs_approval {
                    return Err(TransitionError::ApprovalNotRequired);
                }
                if !meta.ts.is_before(&expires_at) {
                    return Err(TransitionError::ExpiryNotInFuture);
                }
                let data = ApprovalRequested {
                    approval_id: approval_id.clone(),
                    plan_hash: self.plan_hash.clone(),
                    risk: self.risk,
                    summary,
                    expires_at: expires_at.clone(),
                    needs_elevation: false,
                };
                Ok((
                    self.with(State::AwaitingApproval {
                        approval_id,
                        expires_at,
                    }),
                    Event::ApprovalRequested(Envelope::new(meta, data)),
                ))
            }
            Command::Grant {
                approval_id,
                plan_hash,
                via,
            } => {
                let expires_at = self.pending("grant", &approval_id, &plan_hash)?;
                if !meta.ts.is_before(expires_at) {
                    return Err(TransitionError::ApprovalLapsed);
                }
                let state = State::Approved {
                    approval_id: approval_id.clone(),
                    expires_at: expires_at.clone(),
                };
                let data = ApprovalGranted {
                    approval_id,
                    plan_hash,
                    via,
                };
                Ok((
                    self.with(state),
                    Event::ApprovalGranted(Envelope::new(meta, data)),
                ))
            }
            Command::Deny {
                approval_id,
                plan_hash,
                reason,
            } => {
                self.pending("deny", &approval_id, &plan_hash)?;
                let data = ApprovalDenied {
                    approval_id,
                    plan_hash,
                    reason,
                };
                Ok((
                    self.with(State::ApprovalDenied),
                    Event::ApprovalDenied(Envelope::new(meta, data)),
                ))
            }
            Command::Expire {
                approval_id,
                plan_hash,
            } => {
                let expires_at = self.pending("expire", &approval_id, &plan_hash)?;
                if meta.ts.is_before(expires_at) {
                    return Err(TransitionError::NotYetExpired);
                }
                let data = ApprovalExpired {
                    approval_id,
                    plan_hash,
                };
                Ok((
                    self.with(State::ApprovalExpired),
                    Event::ApprovalExpired(Envelope::new(meta, data)),
                ))
            }
            Command::Executed { permit } => {
                let State::Executing { approval_id } = &self.state else {
                    return Err(self.invalid("executed"));
                };
                if permit.action_id != self.action_id {
                    return Err(TransitionError::ActionMismatch);
                }
                if permit.plan_hash != self.plan_hash || permit.plan.hash() != self.plan_hash {
                    return Err(TransitionError::PlanHashMismatch);
                }
                if &permit.approval_id != approval_id {
                    return Err(TransitionError::ApprovalMismatch);
                }
                let data = ActionExecuted {
                    action_id: self.action_id.clone(),
                    plan_hash: self.plan_hash.clone(),
                    approval_id: permit.approval_id,
                };
                Ok((
                    self.with(State::Executed),
                    Event::ActionExecuted(Envelope::new(meta, data)),
                ))
            }
            Command::Fail { error_code } => {
                // A pending approval has to be denied or to expire first.
                if !matches!(
                    self.state,
                    State::Planned | State::Approved { .. } | State::Executing { .. }
                ) {
                    return Err(self.invalid("fail"));
                }
                let data = ActionFailed {
                    action_id: self.action_id.clone(),
                    plan_hash: self.plan_hash.clone(),
                    error_code,
                };
                Ok((
                    self.with(State::Failed),
                    Event::ActionFailed(Envelope::new(meta, data)),
                ))
            }
        }
    }
}
