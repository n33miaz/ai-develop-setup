use std::collections::{HashMap, HashSet};
use std::fmt;

use super::{
    AbsPath, Action, ActionPlan, ActionPlanRecord, ActionSnapshot, ClassificationBudgetExceeded,
    Command, Decision, ErrorCode, Event, EventMeta, ExecutionPermit, GatePolicy, Id,
    OperationRequest, PlanOperation, PlanRequest, ReplayError, ShortText, State, TargetRequest,
    TransitionError, Verdict, classify,
};

/// A complete, authenticated action history in commit order.
pub struct GateHistory {
    pub plan: ActionPlanRecord,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    Missing,
    Conflict,
    Integrity,
    PolicyChanged,
    PlanNormalizationFailed,
    ClassificationBudgetExceeded(ClassificationBudgetExceeded),
    Transition,
    /// The incoming timestamp is too early. Retry with a current timestamp.
    OutOfOrder,
    /// A recorded timestamp exceeds the configured rewind limit.
    HistoryOutOfOrder,
    Unavailable,
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gate journal: {self:?}")
    }
}

impl std::error::Error for JournalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ClassificationBudgetExceeded(error) => Some(error),
            _ => None,
        }
    }
}

pub trait GateJournal {
    /// Loads and authenticates the complete action history against the durable log.
    fn load(&mut self, action_id: &Id) -> Result<GateHistory, JournalError>;

    /// Atomically creates the action with its plan and first event. An action id can be
    /// created only once, and success means both records are durable.
    fn create(&mut self, plan: &ActionPlanRecord, first: &Event) -> Result<(), JournalError>;

    /// Appends only if the authenticated history still has this number of events.
    /// Success requires durability, globally unique event ids, contiguous stream sequences,
    /// exclusive approval ownership and at most one start per action.
    fn append(
        &mut self,
        action_id: &Id,
        expected_events: usize,
        event: &Event,
    ) -> Result<(), JournalError>;

    /// Loads the authenticated history, lets `decide` build the next event from it and
    /// appends that event with the extension `decide` saw. An implementation that can should
    /// do the three under one lock, with one authentication of the log; the default is
    /// `load` followed by `append`, whose extension check still refuses a concurrent write.
    fn transition(
        &mut self,
        action_id: &Id,
        decide: &mut dyn FnMut(&GateHistory) -> Result<Event, JournalError>,
    ) -> Result<(), JournalError> {
        let history = self.load(action_id)?;
        let event = decide(&history)?;
        self.append(action_id, history.events.len(), &event)
    }
}

/// One permit whose start was accepted by the journal. Consuming it keeps the
/// executor from treating recovery of the start event as a new authorization.
#[derive(Debug, PartialEq, Eq)]
pub struct StartedExecution {
    permit: ExecutionPermit,
    event: Event,
}

impl StartedExecution {
    pub fn permit(&self) -> &ExecutionPermit {
        &self.permit
    }

    pub fn into_parts(self) -> (ExecutionPermit, Event) {
        (self.permit, self.event)
    }
}

pub struct JournalGate<'a> {
    policy: &'a GatePolicy,
    journal: &'a mut dyn GateJournal,
    work_root: Option<&'a AbsPath>,
}

impl<'a> JournalGate<'a> {
    pub fn new(policy: &'a GatePolicy, journal: &'a mut dyn GateJournal) -> Self {
        Self {
            policy,
            journal,
            work_root: None,
        }
    }

    /// Restricts access to actions with the session folder recorded in their plan.
    pub fn for_work_root(mut self, root: &'a AbsPath) -> Self {
        self.work_root = Some(root);
        self
    }

    pub fn plan(
        &mut self,
        action_id: Id,
        plan: &ActionPlan,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        check_work_root(self.work_root, plan)?;
        let verdict =
            classify(plan, self.policy).map_err(JournalError::ClassificationBudgetExceeded)?;
        let (action, first, record) = Action::plan(action_id.clone(), &verdict, meta)
            .map_err(|_| JournalError::Transition)?;
        validate_envelopes(std::slice::from_ref(&first))?;
        self.journal.create(&record, &first)?;
        Ok(action.snapshot())
    }

    pub fn inspect(&mut self, action_id: &Id) -> Result<ActionSnapshot, JournalError> {
        let history = self.journal.load(action_id)?;
        let mut snapshot =
            load_action(self.policy, self.work_root, action_id, &history)?.snapshot();
        snapshot.policy_warning = match current_verdict(self.policy, &history.plan) {
            Ok(_) => None,
            Err(
                error @ (JournalError::PolicyChanged
                | JournalError::PlanNormalizationFailed
                | JournalError::ClassificationBudgetExceeded(_)),
            ) => Some(error),
            Err(error) => return Err(error),
        };
        Ok(snapshot)
    }

    /// Applies a command. A grant invalidated by policy records a denial before returning
    /// [`JournalError::PolicyChanged`].
    pub fn apply(
        &mut self,
        action_id: &Id,
        command: Command,
        meta: EventMeta,
    ) -> Result<ActionSnapshot, JournalError> {
        let policy = self.policy;
        let work_root = self.work_root;
        let mut input = Some((command, meta));
        let mut next = None;
        self.journal.transition(action_id, &mut |history| {
            let (command, meta) = input.take().ok_or(JournalError::Transition)?;
            let action = load_action(policy, work_root, action_id, history)?;
            let (updated, event) = action.try_apply(command, meta).map_err(transition_error)?;
            if matches!(&event, Event::ApprovalRequested(_)) {
                current_verdict(policy, &history.plan)?;
            }
            if let Event::ApprovalGranted(granted) = &event {
                match current_verdict(policy, &history.plan) {
                    Err(JournalError::PolicyChanged) => {
                        let data = granted.data();
                        let denial = Command::Deny {
                            action_id: data.action_id.clone(),
                            approval_id: data.approval_id.clone(),
                            plan_hash: data.plan_hash.clone(),
                            reason: Some(ShortText::new("E_POLICY_CHANGED").expect("valid reason")),
                        };
                        let (_, denied) = action
                            .try_apply(denial, event.meta())
                            .map_err(transition_error)?;
                        validate_next(history, &denied)?;
                        next = Some(Err(JournalError::PolicyChanged));
                        return Ok(denied);
                    }
                    result => {
                        result?;
                    }
                }
            }
            validate_next(history, &event)?;
            next = Some(Ok(updated.snapshot()));
            Ok(event)
        })?;
        next.ok_or(JournalError::Transition)?
    }

    /// Starts an eligible action, or records its failure before returning
    /// [`JournalError::PolicyChanged`] when its current verdict differs.
    pub fn authorize(
        &mut self,
        action_id: &Id,
        meta: EventMeta,
    ) -> Result<StartedExecution, JournalError> {
        let policy = self.policy;
        let work_root = self.work_root;
        let mut input = Some(meta);
        let mut started = None;
        self.journal.transition(action_id, &mut |history| {
            let meta = input.take().ok_or(JournalError::Transition)?;
            let action = load_action(policy, work_root, action_id, history)?;
            if !matches!(action.state(), State::Planned | State::Approved { .. }) {
                return Err(JournalError::Transition);
            }
            let verdict = match current_verdict(policy, &history.plan) {
                Err(JournalError::PolicyChanged) => {
                    let command = Command::Fail {
                        error_code: ErrorCode::new("E_POLICY_CHANGED").expect("valid error code"),
                    };
                    let (_, event) = action
                        .apply(command, meta)
                        .map_err(|refused| transition_error(refused.error))?;
                    validate_next(history, &event)?;
                    started = Some(Err(JournalError::PolicyChanged));
                    return Ok(event);
                }
                result => result?,
            };
            let (_, permit, event) = action
                .authorize(verdict.plan(), meta)
                .map_err(|refused| transition_error(refused.error))?;
            validate_next(history, &event)?;
            started = Some(Ok(StartedExecution {
                permit,
                event: event.clone(),
            }));
            Ok(event)
        })?;
        started.ok_or(JournalError::Transition)?
    }
}

fn transition_error(error: TransitionError) -> JournalError {
    match error {
        TransitionError::OutOfOrder => JournalError::OutOfOrder,
        _ => JournalError::Transition,
    }
}

fn validate_next(history: &GateHistory, event: &Event) -> Result<(), JournalError> {
    let mut events = history.events.clone();
    events.push(event.clone());
    validate_envelopes(&events)
}

fn validate_envelopes(events: &[Event]) -> Result<(), JournalError> {
    let mut ids = HashSet::new();
    let mut sequences = HashMap::new();
    for event in events {
        let sequence = sequences.entry(event.stream()).or_insert(0_u64);
        if !ids.insert(event.id().as_str()) || sequence.checked_add(1) != Some(event.seq().get()) {
            return Err(JournalError::Integrity);
        }
        *sequence = event.seq().get();
    }
    Ok(())
}

fn check_work_root(root: Option<&AbsPath>, plan: &ActionPlan) -> Result<(), JournalError> {
    if root.is_some_and(|root| root != plan.work_root()) {
        return Err(JournalError::Missing);
    }
    Ok(())
}

fn load_action(
    policy: &GatePolicy,
    work_root: Option<&AbsPath>,
    action_id: &Id,
    history: &GateHistory,
) -> Result<Action, JournalError> {
    validate_envelopes(&history.events)?;
    let (first, rest) = history
        .events
        .split_first()
        .ok_or(JournalError::Integrity)?;
    let record = history.plan.fields();
    if &record.action_id != action_id {
        return Err(JournalError::Integrity);
    }
    let recorded_root =
        AbsPath::parse(record.work_root.as_str()).map_err(|_| JournalError::Integrity)?;
    if work_root.is_some_and(|root| root != &recorded_root) {
        return Err(JournalError::Missing);
    }

    let verb = if record.quarantined {
        PlanOperation::Move
    } else {
        record.operation
    };
    let matches_record = match first {
        Event::ActionPlanned(event) => {
            let data = event.data();
            record.verdict.decision != Decision::Denied
                && data.action_id == record.action_id
                && data.plan_hash == record.plan_hash
                && data.verb.as_str() == verb.as_str()
                && data.risk == record.verdict.risk
        }
        Event::ActionDenied(event) => {
            let data = event.data();
            record.verdict.decision == Decision::Denied
                && data.action_id == record.action_id
                && data.plan_hash == record.plan_hash
                && data.verb.as_str() == verb.as_str()
                && data.rules.as_slice().iter().all(|rule| {
                    record
                        .verdict
                        .reasons
                        .iter()
                        .any(|reason| &reason.rule == rule)
                })
        }
        _ => false,
    };
    if !matches_record {
        return Err(JournalError::Integrity);
    }
    let mut action = Action::begin(first, policy.gate_clock_rewind_seconds())
        .map_err(|_| JournalError::Integrity)?;
    for event in rest {
        action = action.after(event).map_err(|error| match error {
            ReplayError::OutOfOrder => JournalError::HistoryOutOfOrder,
            _ => JournalError::Integrity,
        })?;
    }
    Ok(action)
}

fn current_verdict(
    policy: &GatePolicy,
    plan_record: &ActionPlanRecord,
) -> Result<Verdict, JournalError> {
    let record = plan_record.fields();
    let operation = match record.operation {
        PlanOperation::Delete => OperationRequest::Delete,
        PlanOperation::DeleteRecursive => OperationRequest::DeleteRecursive,
        PlanOperation::Move => OperationRequest::Move {
            destination: record
                .destination
                .as_ref()
                .ok_or(JournalError::Integrity)?
                .as_str()
                .to_owned(),
        },
        PlanOperation::Overwrite => OperationRequest::Overwrite,
        PlanOperation::CleanUntracked => OperationRequest::CleanUntracked,
    };
    let plan = ActionPlan::normalize(&PlanRequest {
        operation,
        work_root: record.work_root.as_str().to_owned(),
        targets: record
            .targets
            .iter()
            .map(|target| TargetRequest {
                path: target.text(),
                repo_root: target.repo_root,
            })
            .collect(),
        estimate: record.estimate.map(|estimate| super::Estimate {
            items: estimate.items,
            bytes: estimate.bytes,
        }),
    })
    .map_err(|_| JournalError::PlanNormalizationFailed)?;
    let verdict = classify(&plan, policy).map_err(JournalError::ClassificationBudgetExceeded)?;
    if !verdict
        .record(record.action_id.clone())
        .is_ok_and(|current| &current == plan_record)
    {
        return Err(JournalError::PolicyChanged);
    }
    Ok(verdict)
}
