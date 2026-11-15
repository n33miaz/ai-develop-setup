//! The life of one action: planned or denied, approved when the verdict asks for it, started,
//! executed or failed. Every transition is a pure function that refuses any jump and yields the
//! contract event; the state after an event is computed from the event alone, so the log
//! rebuilds it ([`replay`]).

use std::fmt;

use super::classify::Verdict;
use super::event::{
    ActionDenied, ActionExecuted, ActionFailed, ActionPlanRecord, ActionPlanned, ActionStarted,
    ApprovalDenied, ApprovalExpired, ApprovalGranted, ApprovalRequested, ContractError, Envelope,
    ErrorCode, Event, EventMeta, Id, PlanHash, Risk, ShortText, Timestamp, Via,
};
use super::plan::ActionPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// The gate denied the plan: nothing runs and no approval fits.
    Denied,
    Planned,
    AwaitingApproval {
        approval_id: Id,
        expires_at: Timestamp,
    },
    Approved {
        approval_id: Id,
        expires_at: Timestamp,
    },
    /// The execution permit was handed out.
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
            Self::Denied
                | Self::ApprovalDenied
                | Self::ApprovalExpired
                | Self::Executed
                | Self::Failed
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
    /// The plan does not fit the limits of the `record.action-plan` contract: with no record
    /// there is no action and no event.
    Unrecordable(ContractError),
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
    /// The event of the transition does not follow from the state. The checks of each command
    /// come first, so this is a defect of the machine, kept as an error instead of a panic.
    Inconsistent(ReplayError),
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unrecordable(error) => write!(f, "the plan cannot be recorded: {error}"),
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
            Self::Inconsistent(error) => write!(f, "the event does not follow: {error}"),
        }
    }
}

/// Why a sequence of events is not the log of one action of this gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    EmptyLog,
    /// The event does not follow from the state; `None` is the state before the first event,
    /// which accepts only `action.planned` and `action.denied`.
    UnexpectedEvent {
        state: Option<State>,
        event: String,
    },
    /// The event names another action.
    ActionMismatch,
    PlanHashMismatch,
    /// The event names another approval than the one of the action.
    ApprovalMismatch,
    /// `action.started` without the approval the plan needs.
    ApprovalRequired,
    /// The instant of the event is on the wrong side of the validity of the approval.
    OutsideValidity,
    /// A valid event this gate never writes: a planned action of high risk or asking for
    /// elevation, or an approval asked for a plan that needs none or with another risk.
    NotFromThisGate(&'static str),
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLog => f.write_str("no events"),
            Self::UnexpectedEvent { state, event } => match state {
                Some(state) => write!(f, "`{event}` does not follow state {state:?}"),
                None => write!(f, "`{event}` cannot be the first event of an action"),
            },
            Self::ActionMismatch => f.write_str("another action"),
            Self::PlanHashMismatch => f.write_str("another plan hash"),
            Self::ApprovalMismatch => f.write_str("another approval"),
            Self::ApprovalRequired => f.write_str("started without the approval the plan needs"),
            Self::OutsideValidity => f.write_str("outside the validity of the approval"),
            Self::NotFromThisGate(what) => write!(f, "not written by this gate: {what}"),
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

/// What the log says about one action. Read only: it shows the state and hands out no permit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionSnapshot {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub state: State,
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
    /// one) and writes its record. A denied verdict yields `action.denied` and an action born
    /// in its final state: no command and no permit ever come out of it.
    pub fn plan(
        action_id: Id,
        verdict: &Verdict,
        meta: EventMeta,
    ) -> Result<(Action, Event, ActionPlanRecord), TransitionError> {
        let record = verdict
            .record(action_id.clone())
            .map_err(TransitionError::Unrecordable)?;
        let plan_hash = record.fields().plan_hash.clone();
        let event = match verdict.denying_rules() {
            Some(rules) => Event::ActionDenied(Envelope::new(
                meta,
                ActionDenied {
                    action_id,
                    verb: verdict.plan().operation().verb(),
                    plan_hash,
                    rules,
                },
            )),
            None => Event::ActionPlanned(Envelope::new(
                meta,
                ActionPlanned {
                    action_id,
                    verb: verdict.plan().operation().verb(),
                    risk: verdict.risk(),
                    plan_hash,
                    // The core never runs elevated and no rule here asks for it.
                    needs_elevation: false,
                },
            )),
        };
        let action = Action::begin(&event).map_err(TransitionError::Inconsistent)?;
        Ok((action, event, record))
    }

    /// The action right after its first event.
    fn begin(event: &Event) -> Result<Action, ReplayError> {
        match event {
            Event::ActionPlanned(event) => {
                let data = event.data();
                let needs_approval = match data.risk {
                    Risk::Low => false,
                    Risk::Medium => true,
                    Risk::High => {
                        return Err(ReplayError::NotFromThisGate(
                            "a plan of high risk is a denied one",
                        ));
                    }
                };
                if data.needs_elevation {
                    return Err(ReplayError::NotFromThisGate("a plan asking for elevation"));
                }
                Ok(Action {
                    action_id: data.action_id.clone(),
                    plan_hash: data.plan_hash.clone(),
                    risk: data.risk,
                    needs_approval,
                    state: State::Planned,
                })
            }
            Event::ActionDenied(event) => Ok(Action {
                action_id: event.data().action_id.clone(),
                plan_hash: event.data().plan_hash.clone(),
                risk: Risk::High,
                needs_approval: false,
                state: State::Denied,
            }),
            other => Err(ReplayError::UnexpectedEvent {
                state: None,
                event: other.kind().to_owned(),
            }),
        }
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

    /// What [`replay`] must give back for the events of this action so far.
    pub fn snapshot(&self) -> ActionSnapshot {
        ActionSnapshot {
            action_id: self.action_id.clone(),
            plan_hash: self.plan_hash.clone(),
            risk: self.risk,
            state: self.state.clone(),
        }
    }

    fn invalid(&self, command: &'static str) -> TransitionError {
        TransitionError::InvalidTransition {
            state: self.state.clone(),
            command,
        }
    }

    /// The action after `event`, from the event alone. The only place a state changes: the
    /// commands go through here, and so does [`replay`].
    fn after(&self, event: &Event) -> Result<Action, ReplayError> {
        let same_action = |action_id: &Id| {
            if action_id == &self.action_id {
                Ok(())
            } else {
                Err(ReplayError::ActionMismatch)
            }
        };
        let same_plan = |plan_hash: &PlanHash| {
            if plan_hash == &self.plan_hash {
                Ok(())
            } else {
                Err(ReplayError::PlanHashMismatch)
            }
        };
        let same_approval = |named: Option<&Id>, expected: Option<&Id>| {
            if named == expected {
                Ok(())
            } else {
                Err(ReplayError::ApprovalMismatch)
            }
        };
        let within = |ts: &Timestamp, expires_at: &Timestamp, before: bool| {
            if ts.is_before(expires_at) == before {
                Ok(())
            } else {
                Err(ReplayError::OutsideValidity)
            }
        };

        let state = match (&self.state, event) {
            (State::Planned, Event::ApprovalRequested(event)) => {
                let data = event.data();
                same_action(&data.action_id)?;
                same_plan(&data.plan_hash)?;
                if !self.needs_approval {
                    return Err(ReplayError::NotFromThisGate(
                        "an approval asked for a plan that needs none",
                    ));
                }
                if data.risk != self.risk || data.needs_elevation {
                    return Err(ReplayError::NotFromThisGate(
                        "an approval with another risk than its plan",
                    ));
                }
                within(event.ts(), &data.expires_at, true)?;
                State::AwaitingApproval {
                    approval_id: data.approval_id.clone(),
                    expires_at: data.expires_at.clone(),
                }
            }
            (
                State::AwaitingApproval {
                    approval_id,
                    expires_at,
                },
                Event::ApprovalGranted(event),
            ) => {
                same_approval(Some(&event.data().approval_id), Some(approval_id))?;
                same_plan(&event.data().plan_hash)?;
                within(event.ts(), expires_at, true)?;
                State::Approved {
                    approval_id: approval_id.clone(),
                    expires_at: expires_at.clone(),
                }
            }
            (State::AwaitingApproval { approval_id, .. }, Event::ApprovalDenied(event)) => {
                same_approval(Some(&event.data().approval_id), Some(approval_id))?;
                same_plan(&event.data().plan_hash)?;
                State::ApprovalDenied
            }
            (
                State::AwaitingApproval {
                    approval_id,
                    expires_at,
                },
                Event::ApprovalExpired(event),
            ) => {
                same_approval(Some(&event.data().approval_id), Some(approval_id))?;
                same_plan(&event.data().plan_hash)?;
                within(event.ts(), expires_at, false)?;
                State::ApprovalExpired
            }
            (State::Planned | State::AwaitingApproval { .. }, Event::ActionStarted(event))
                if self.needs_approval =>
            {
                same_action(&event.data().action_id)?;
                return Err(ReplayError::ApprovalRequired);
            }
            (State::Planned, Event::ActionStarted(event)) => {
                let data = event.data();
                same_action(&data.action_id)?;
                same_plan(&data.plan_hash)?;
                same_approval(data.approval_id.as_ref(), None)?;
                State::Executing { approval_id: None }
            }
            (
                State::Approved {
                    approval_id,
                    expires_at,
                },
                Event::ActionStarted(event),
            ) => {
                let data = event.data();
                same_action(&data.action_id)?;
                same_plan(&data.plan_hash)?;
                same_approval(data.approval_id.as_ref(), Some(approval_id))?;
                within(event.ts(), expires_at, true)?;
                State::Executing {
                    approval_id: Some(approval_id.clone()),
                }
            }
            (State::Executing { approval_id }, Event::ActionExecuted(event)) => {
                let data = event.data();
                same_action(&data.action_id)?;
                same_plan(&data.plan_hash)?;
                same_approval(data.approval_id.as_ref(), approval_id.as_ref())?;
                State::Executed
            }
            // A pending approval has to be denied or to expire first.
            (
                State::Planned | State::Approved { .. } | State::Executing { .. },
                Event::ActionFailed(event),
            ) => {
                same_action(&event.data().action_id)?;
                same_plan(&event.data().plan_hash)?;
                State::Failed
            }
            (state, event) => {
                return Err(ReplayError::UnexpectedEvent {
                    state: Some(state.clone()),
                    event: event.kind().to_owned(),
                });
            }
        };
        Ok(Action {
            state,
            ..self.clone()
        })
    }

    /// The transition of `event`, already checked by the command that built it.
    fn step(&self, event: Event) -> Result<(Action, Event), TransitionError> {
        let action = self.after(&event).map_err(TransitionError::Inconsistent)?;
        Ok((action, event))
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

    /// Hands out the permit to execute `plan` at the instant `meta.ts`, moves to `Executing`
    /// and yields `action.started` (`meta.seq` counts in `action/<id>`). Refused when the plan
    /// is not the one planned, when the verdict asked for an approval that was not granted, or
    /// when the granted approval is no longer valid.
    pub fn authorize(
        &self,
        plan: &ActionPlan,
        meta: EventMeta,
    ) -> Result<(Action, ExecutionPermit, Event), TransitionError> {
        let approval_id = match &self.state {
            State::Planned if !self.needs_approval => None,
            State::Planned | State::AwaitingApproval { .. } => {
                return Err(TransitionError::ApprovalRequired);
            }
            State::Approved {
                approval_id,
                expires_at,
            } => {
                if !meta.ts.is_before(expires_at) {
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
        let data = ActionStarted {
            action_id: self.action_id.clone(),
            plan_hash: plan_hash.clone(),
            approval_id: approval_id.clone(),
        };
        let (action, event) = self.step(Event::ActionStarted(Envelope::new(meta, data)))?;
        let permit = ExecutionPermit {
            action_id: self.action_id.clone(),
            plan_hash,
            approval_id,
            plan: plan.clone(),
        };
        Ok((action, permit, event))
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
                    approval_id,
                    action_id: self.action_id.clone(),
                    plan_hash: self.plan_hash.clone(),
                    risk: self.risk,
                    summary,
                    expires_at,
                    needs_elevation: false,
                };
                self.step(Event::ApprovalRequested(Envelope::new(meta, data)))
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
                let data = ApprovalGranted {
                    approval_id,
                    plan_hash,
                    via,
                };
                self.step(Event::ApprovalGranted(Envelope::new(meta, data)))
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
                self.step(Event::ApprovalDenied(Envelope::new(meta, data)))
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
                self.step(Event::ApprovalExpired(Envelope::new(meta, data)))
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
                self.step(Event::ActionExecuted(Envelope::new(meta, data)))
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
                self.step(Event::ActionFailed(Envelope::new(meta, data)))
            }
        }
    }
}

/// Rebuilds the state of one action from its events alone, in the order they were written:
/// the `action.*` events of `action/<id>` and the `approval.*` events of the approval that
/// `approval.requested` ties to it. Refuses whatever the machine would not have written: an
/// empty log, a jump, an event after a final state, another action, plan hash or approval.
pub fn replay(events: &[Event]) -> Result<ActionSnapshot, ReplayError> {
    let (first, rest) = events.split_first().ok_or(ReplayError::EmptyLog)?;
    let mut action = Action::begin(first)?;
    for event in rest {
        action = action.after(event)?;
    }
    Ok(action.snapshot())
}
