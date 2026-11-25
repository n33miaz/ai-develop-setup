//! Authorization and lifecycle policy for the task queue.

use std::collections::{HashMap, HashSet};
use std::num::{NonZeroU64, NonZeroUsize};

use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonIdV1, CommonQueuePolicyV1,
    CommonQueuePolicyV1AttemptLimit, CommonQueueTaskOriginV1, CommonQueueTaskOriginV1BugRule,
    CommonQueueTaskOriginV1Issue, CommonQueueTaskOriginV1IssueRef, CommonQueueTaskOriginV1Manual,
    CommonQueueTaskStateV1, CommonQueueTaskTitleV1, CommonTimestampV1, CommonWorkdirAliasV1,
    EventEnvelopeV1, EventQueueClockReanchoredV1, EventQueueClockReanchoredV1Data,
    EventQueueClockReanchoredV1DataVia, EventQueueCreatedV1, EventQueueCreatedV2,
    EventQueueCreatedV2Data, EventQueueCreatedV2DataState, EventQueueCreatedV2DataVia,
    EventQueueEnabledV1, EventQueueEnabledV1Data, EventQueueEnabledV1DataVia,
    EventQueuePolicyChangedV1, EventQueuePolicyChangedV1Data, EventQueuePolicyChangedV1DataVia,
    EventQueueStoppedV1, EventQueueStoppedV1Data, EventQueueStoppedV1DataReason,
    EventQueueStoppedV1DataVia, EventQueueTaskCreatedV1, EventQueueTaskCreatedV1DataOrigin,
    EventQueueTaskCreatedV2, EventQueueTaskCreatedV2DataOrigin, EventQueueTaskCreatedV3,
    EventQueueTaskCreatedV4, EventQueueTaskStateChangedV1, EventQueueTaskStateChangedV2,
    EventQueueTaskStateChangedV2Data, EventQueueTaskStateChangedV2DataAttempts,
    EventQueueTaskStateChangedV2DataFields, EventQueueTaskStateChangedV2DataReason,
    EventQueueTaskStateChangedV2DataVia, RecordQueueTaskV1, RecordQueueTaskV2,
    RecordQueueTaskV2Attempts, RecordQueueTaskV3, RecordQueueTaskV3AttemptLimit,
    RecordQueueTaskV3Attempts, RecordQueueTaskV3Version, RecordTaskQueueV1, RecordTaskQueueV1State,
    RecordTaskQueueV1Version,
};

use crate::device::check_owner_device;
use crate::timestamp::epoch_seconds;

use CommonQueueTaskStateV1::{Cancelled, Done, Failed, Queued, Review, Running};
use RecordTaskQueueV1State::{Enabled, Stopped};

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/queue.toml");

/// Limits of the queue. Keys missing from an older file take the values of `defaults/queue.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuePolicy {
    pub attempt_limit: ads_contracts::RecordQueueTaskV2AttemptLimit,
    pub admission_limit: NonZeroUsize,
    #[serde(default = "default_window_seconds")]
    pub window_seconds: NonZeroU64,
    #[serde(default = "default_window_admissions")]
    pub window_admissions: NonZeroUsize,
    #[serde(default = "default_manual_window_admissions")]
    pub manual_window_admissions: NonZeroUsize,
    #[serde(default = "default_bug_rule_window_admissions")]
    pub bug_rule_window_admissions: NonZeroUsize,
    #[serde(default = "default_clock_skew_seconds")]
    pub clock_skew_seconds: u64,
    #[serde(default = "default_clock_rewind_seconds")]
    pub clock_rewind_seconds: u64,
    #[serde(default = "default_host_confirm_seconds")]
    pub host_confirm_seconds: NonZeroU64,
}

impl QueuePolicy {
    fn from_event(policy: &CommonQueuePolicyV1) -> Option<Self> {
        Some(Self {
            attempt_limit: ads_contracts::RecordQueueTaskV2AttemptLimit::new(
                policy.attempt_limit.get(),
            )
            .ok()?,
            admission_limit: NonZeroUsize::new(policy.admission_limit.get().try_into().ok()?)?,
            window_seconds: policy.window_seconds,
            window_admissions: NonZeroUsize::new(policy.window_admissions.get().try_into().ok()?)?,
            manual_window_admissions: NonZeroUsize::new(
                policy.manual_window_admissions.get().try_into().ok()?,
            )?,
            bug_rule_window_admissions: NonZeroUsize::new(
                policy.bug_rule_window_admissions.get().try_into().ok()?,
            )?,
            clock_skew_seconds: policy.clock_skew_seconds,
            clock_rewind_seconds: policy.clock_rewind_seconds,
            host_confirm_seconds: policy.host_confirm_seconds,
        })
    }

    fn event_data(&self) -> CommonQueuePolicyV1 {
        let encode_count =
            |count: NonZeroUsize| NonZeroU64::new(count.get() as u64).expect("nonzero queue count");
        CommonQueuePolicyV1 {
            attempt_limit: CommonQueuePolicyV1AttemptLimit::new(self.attempt_limit.get())
                .expect("queue policies share the attempt limit range"),
            admission_limit: encode_count(self.admission_limit),
            window_seconds: self.window_seconds,
            window_admissions: encode_count(self.window_admissions),
            manual_window_admissions: encode_count(self.manual_window_admissions),
            bug_rule_window_admissions: encode_count(self.bug_rule_window_admissions),
            clock_skew_seconds: self.clock_skew_seconds,
            clock_rewind_seconds: self.clock_rewind_seconds,
            host_confirm_seconds: self.host_confirm_seconds,
        }
    }
}

/// Reads one key of the bundled defaults, which a test loads in full.
fn bundled_default<T: serde::de::DeserializeOwned>(key: &str) -> T {
    let table: toml::Table = toml::from_str(DEFAULT_POLICY).expect("defaults/queue.toml parses");
    table
        .get(key)
        .cloned()
        .and_then(|value| value.try_into().ok())
        .expect("defaults/queue.toml has every queue key")
}

fn default_window_seconds() -> NonZeroU64 {
    bundled_default("window_seconds")
}

fn default_window_admissions() -> NonZeroUsize {
    bundled_default("window_admissions")
}

fn default_manual_window_admissions() -> NonZeroUsize {
    bundled_default("manual_window_admissions")
}

fn default_bug_rule_window_admissions() -> NonZeroUsize {
    bundled_default("bug_rule_window_admissions")
}

fn default_clock_skew_seconds() -> u64 {
    bundled_default("clock_skew_seconds")
}

fn default_clock_rewind_seconds() -> u64 {
    bundled_default("clock_rewind_seconds")
}

fn default_host_confirm_seconds() -> NonZeroU64 {
    bundled_default("host_confirm_seconds")
}

/// Channel through which a device acted, independent of the event that records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    LocalUi,
    Tray,
    RemoteDevice,
}

impl Channel {
    pub fn created_via(self) -> EventQueueCreatedV2DataVia {
        match self {
            Self::LocalUi => EventQueueCreatedV2DataVia::LocalUi,
            Self::Tray => EventQueueCreatedV2DataVia::Tray,
            Self::RemoteDevice => EventQueueCreatedV2DataVia::RemoteDevice,
        }
    }

    /// `None` for the remote channel, which never enables a queue.
    pub fn enabled_via(self) -> Option<EventQueueEnabledV1DataVia> {
        match self {
            Self::LocalUi => Some(EventQueueEnabledV1DataVia::LocalUi),
            Self::Tray => Some(EventQueueEnabledV1DataVia::Tray),
            Self::RemoteDevice => None,
        }
    }

    pub fn stopped_via(self) -> EventQueueStoppedV1DataVia {
        match self {
            Self::LocalUi => EventQueueStoppedV1DataVia::LocalUi,
            Self::Tray => EventQueueStoppedV1DataVia::Tray,
            Self::RemoteDevice => EventQueueStoppedV1DataVia::RemoteDevice,
        }
    }

    pub fn task_via(self) -> EventQueueTaskStateChangedV2DataVia {
        match self {
            Self::LocalUi => EventQueueTaskStateChangedV2DataVia::LocalUi,
            Self::Tray => EventQueueTaskStateChangedV2DataVia::Tray,
            Self::RemoteDevice => EventQueueTaskStateChangedV2DataVia::RemoteDevice,
        }
    }
}

/// Device, the owner it is registered to and channel, established by the authentication boundary.
///
/// There is deliberately no public constructor. Any public function of this crate can be called
/// by every crate that depends on it, and the dependency matrix keeps the authentication layer
/// out of this crate, so no constructor here could tell the authentication layer from a caller
/// that copies a device id out of a request. The constructor arrives together with that layer.
///
/// ```compile_fail
/// use ads_contracts::CommonIdV1;
/// let id = CommonIdV1::new("owner-01").unwrap();
/// let _ = ads_core::queue::AuthenticatedDevice::local_owner(id.clone(), id);
/// ```
pub struct AuthenticatedDevice {
    device_id: CommonIdV1,
    owner_id: CommonIdV1,
    via: Channel,
}

impl AuthenticatedDevice {
    pub fn device_id(&self) -> &CommonIdV1 {
        &self.device_id
    }

    pub fn owner_id(&self) -> &CommonIdV1 {
        &self.owner_id
    }

    pub fn via(&self) -> Channel {
        self.via
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueRefusal {
    Unauthenticated,
    NotOwner,
    RemoteEnable,
    /// A policy change can raise the admission ceilings, so it comes only from a local channel.
    RemotePolicyChange,
    Unchanged,
    /// The `seq` the owner acted on is not the last one of the stream.
    StaleSeq,
}

/// Builds a stopped queue and the data to append as queue.created:2 at sequence 1.
pub fn new_stopped(
    id: CommonIdV1,
    owner: &AuthenticatedDevice,
    policy: &QueuePolicy,
) -> (RecordTaskQueueV1, EventQueueCreatedV2Data) {
    stopped_creation(id, &owner.owner_id, owner.via, policy)
}

/// Builds a queue for a local owner already authenticated by the composition.
pub fn new_local_stopped(
    id: CommonIdV1,
    owner_id: &CommonIdV1,
    policy: &QueuePolicy,
) -> (RecordTaskQueueV1, EventQueueCreatedV2Data) {
    stopped_creation(id, owner_id, Channel::LocalUi, policy)
}

fn stopped_creation(
    id: CommonIdV1,
    owner_id: &CommonIdV1,
    via: Channel,
    policy: &QueuePolicy,
) -> (RecordTaskQueueV1, EventQueueCreatedV2Data) {
    let data = EventQueueCreatedV2Data {
        queue_id: id.clone(),
        owner_id: owner_id.clone(),
        via: via.created_via(),
        state: EventQueueCreatedV2DataState,
        policy: policy.event_data(),
    };
    let queue = RecordTaskQueueV1 {
        version: RecordTaskQueueV1Version,
        id,
        owner_id: owner_id.clone(),
        state: Stopped,
    };
    (queue, data)
}

/// Stops that the daemon records without an owner action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticStop {
    Restart,
    Budget,
    Resources,
}

/// Stops the queue for the daemon; an already stopped queue needs no event.
pub fn stop_automatically(
    queue: &RecordTaskQueueV1,
    cause: AutomaticStop,
) -> (RecordTaskQueueV1, Option<EventQueueStoppedV1Data>) {
    automatic_stop(queue, cause, queue.state == Enabled)
}

fn automatic_stop(
    queue: &RecordTaskQueueV1,
    cause: AutomaticStop,
    was_enabled: bool,
) -> (RecordTaskQueueV1, Option<EventQueueStoppedV1Data>) {
    let mut stopped = queue.clone();
    stopped.state = Stopped;
    if !was_enabled {
        return (stopped, None);
    }
    let reason = match cause {
        AutomaticStop::Restart => EventQueueStoppedV1DataReason::Restart,
        AutomaticStop::Budget => EventQueueStoppedV1DataReason::Budget,
        AutomaticStop::Resources => EventQueueStoppedV1DataReason::Resources,
    };
    let data = EventQueueStoppedV1Data {
        queue_id: queue.id.clone(),
        reason,
        owner_id: None,
        via: None,
    };
    (stopped, Some(data))
}

/// Every start restores the queue stopped. The restart is recorded when the snapshot or the
/// last logged queue state says enabled, which covers a crash between the event and the snapshot.
pub fn restore_stopped(
    persisted: RecordTaskQueueV1,
    last_logged: Option<RecordTaskQueueV1State>,
) -> (RecordTaskQueueV1, Option<EventQueueStoppedV1Data>) {
    let was_enabled = persisted.state == Enabled || last_logged == Some(Enabled);
    automatic_stop(&persisted, AutomaticStop::Restart, was_enabled)
}

/// What the terminal host says, when the core starts, about the session of a running task. The
/// caller pairs the task with its session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProcess {
    /// The host lists the session and its process still runs.
    Kept,
    /// The host answered: the session ended or is not listed.
    Lost,
    /// The host has not answered yet, for `waited_seconds` since the core started asking.
    Unconfirmed { waited_seconds: u64 },
}

/// What the start does with a task, with the data of the change to append, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunningAfterRestart {
    /// No change: the task is not running, or the host kept its process and the attempt goes on.
    Keep,
    /// The task stays `running` in the log, but nothing moves it until the host answers or
    /// `host_confirm_seconds` pass: the caller asks again and appends nothing.
    AwaitHost,
    /// `running → failed` with the same count, because the interrupted attempt was already counted.
    Fail(EventQueueTaskStateChangedV2Data),
    /// Terminal cancellation after the host stayed silent for `host_confirm_seconds`.
    /// The recorded reason preserves that the previous session may still be alive.
    Abandon(EventQueueTaskStateChangedV2Data),
    /// `running → cancelled` with the recorded count, for a running task whose count is
    /// inconsistent; a process the host kept, or lists later, is ended by the caller.
    Cancel(EventQueueTaskStateChangedV2Data),
}

/// Fields shared by both persisted versions of a task.
#[derive(Clone, Copy)]
pub struct TaskSnapshot<'a> {
    id: &'a CommonIdV1,
    queue_id: &'a CommonIdV1,
    state: CommonQueueTaskStateV1,
    attempts: u64,
    limit: u64,
}

macro_rules! task_snapshot {
    ($record:ty) => {
        impl<'a> From<&'a $record> for TaskSnapshot<'a> {
            fn from(task: &'a $record) -> Self {
                Self {
                    id: &task.id,
                    queue_id: &task.queue_id,
                    state: task.state,
                    attempts: task.attempts.get(),
                    limit: task.attempt_limit.get().into(),
                }
            }
        }
    };
}

task_snapshot!(RecordQueueTaskV1);
task_snapshot!(RecordQueueTaskV2);
task_snapshot!(RecordQueueTaskV3);

/// Decides, on start, the fate of a task whose last recorded state is `running`. The caller passes
/// the state and count rebuilt by `replay_task`, so a crash between the log and the record is
/// covered.
pub fn restore_running_task<'a>(
    task: impl Into<TaskSnapshot<'a>>,
    host: HostProcess,
    policy: &QueuePolicy,
) -> RunningAfterRestart {
    let task = task.into();
    if task.state != Running {
        return RunningAfterRestart::Keep;
    }
    let attempts = task.attempts;
    let consistent = attempts >= 1 && attempts <= task.limit;
    let abandoned = match (consistent, host) {
        (false, _) => None,
        (true, HostProcess::Kept) => return RunningAfterRestart::Keep,
        (true, HostProcess::Lost) => Some(false),
        (true, HostProcess::Unconfirmed { waited_seconds })
            if waited_seconds < policy.host_confirm_seconds.get() =>
        {
            return RunningAfterRestart::AwaitHost;
        }
        (true, HostProcess::Unconfirmed { .. }) => Some(true),
    };
    let to = if abandoned == Some(false) {
        Failed
    } else {
        Cancelled
    };
    let data = EventQueueTaskStateChangedV2Data::new(EventQueueTaskStateChangedV2DataFields {
        queue_id: task.queue_id.clone(),
        task_id: task.id.clone(),
        from: Running,
        to,
        attempts: EventQueueTaskStateChangedV2DataAttempts::new(attempts)
            .expect("a record count fits the event count"),
        via: None,
        reason: (abandoned == Some(true))
            .then_some(EventQueueTaskStateChangedV2DataReason::HostUnconfirmed),
    })
    .expect("running to failed or cancelled is an allowed change");
    match abandoned {
        Some(false) => RunningAfterRestart::Fail(data),
        Some(true) => RunningAfterRestart::Abandon(data),
        None => RunningAfterRestart::Cancel(data),
    }
}

/// Data of the queue event that records an accepted state change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueStateEvent {
    Enabled(EventQueueEnabledV1Data),
    Stopped(EventQueueStoppedV1Data),
}

/// The returned state, event data and device identify the change to append before scheduling.
/// The event exists only for an actual change by a device of the queue owner, so a stop is
/// never recorded for a queue that is already stopped.
pub fn change_queue_state<'a>(
    queue: &RecordTaskQueueV1,
    device: Option<&'a AuthenticatedDevice>,
    next: RecordTaskQueueV1State,
) -> Result<(RecordTaskQueueV1, QueueStateEvent, &'a AuthenticatedDevice), QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    let (updated, event) = owner_transition(queue, &device.owner_id, device.via, next)?;
    Ok((updated, event, device))
}

/// The same transition for the owner acting through the local interface, whose connection the
/// composition already authenticated. It takes the owner id as data and yields no
/// `AuthenticatedDevice`, so it opens no other owner-only policy; the channel is always `LocalUi`.
pub fn change_local_queue_state(
    queue: &RecordTaskQueueV1,
    owner_id: &CommonIdV1,
    next: RecordTaskQueueV1State,
) -> Result<(RecordTaskQueueV1, QueueStateEvent), QueueRefusal> {
    owner_transition(queue, owner_id, Channel::LocalUi, next)
}

fn owner_transition(
    queue: &RecordTaskQueueV1,
    owner_id: &CommonIdV1,
    via: Channel,
    next: RecordTaskQueueV1State,
) -> Result<(RecordTaskQueueV1, QueueStateEvent), QueueRefusal> {
    if *owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
    }
    if next == Enabled && via.enabled_via().is_none() {
        return Err(QueueRefusal::RemoteEnable);
    }
    if queue.state == next {
        return Err(QueueRefusal::Unchanged);
    }
    let event = match next {
        Enabled => QueueStateEvent::Enabled(EventQueueEnabledV1Data {
            queue_id: queue.id.clone(),
            owner_id: owner_id.clone(),
            via: via.enabled_via().ok_or(QueueRefusal::RemoteEnable)?,
        }),
        Stopped => QueueStateEvent::Stopped(EventQueueStoppedV1Data {
            queue_id: queue.id.clone(),
            reason: EventQueueStoppedV1DataReason::Owner,
            owner_id: Some(owner_id.clone()),
            via: Some(via.stopped_via()),
        }),
    };
    let mut updated = queue.clone();
    updated.state = next;
    Ok((updated, event))
}

/// One event of the `queue/<id>` stream, borrowed for replay.
#[derive(Debug, Clone, Copy)]
pub enum QueueLogEvent<'a> {
    Created(&'a EventQueueCreatedV1),
    CreatedV2(&'a EventQueueCreatedV2),
    Enabled(&'a EventQueueEnabledV1),
    Stopped(&'a EventQueueStoppedV1),
    ClockReanchored(&'a EventQueueClockReanchoredV1),
    PolicyChanged(&'a EventQueuePolicyChangedV1),
    TaskCreated(&'a EventQueueTaskCreatedV1),
    TaskCreatedV2(&'a EventQueueTaskCreatedV2),
    TaskCreatedV3(&'a EventQueueTaskCreatedV3),
    TaskCreatedV4(&'a EventQueueTaskCreatedV4),
    TaskStateChanged(&'a EventQueueTaskStateChangedV1),
    TaskStateChangedV2(&'a EventQueueTaskStateChangedV2),
}

macro_rules! each_event {
    ($event:expr, $name:ident => $body:expr) => {
        match $event {
            QueueLogEvent::Created($name) => $body,
            QueueLogEvent::CreatedV2($name) => $body,
            QueueLogEvent::Enabled($name) => $body,
            QueueLogEvent::Stopped($name) => $body,
            QueueLogEvent::ClockReanchored($name) => $body,
            QueueLogEvent::PolicyChanged($name) => $body,
            QueueLogEvent::TaskCreated($name) => $body,
            QueueLogEvent::TaskCreatedV2($name) => $body,
            QueueLogEvent::TaskCreatedV3($name) => $body,
            QueueLogEvent::TaskCreatedV4($name) => $body,
            QueueLogEvent::TaskStateChanged($name) => $body,
            QueueLogEvent::TaskStateChangedV2($name) => $body,
        }
    };
}

impl<'a> QueueLogEvent<'a> {
    fn seq(self) -> u64 {
        each_event!(self, event => event.seq.get())
    }

    fn stream(self) -> &'a str {
        each_event!(self, event => event.stream.as_str())
    }

    fn queue_id(self) -> &'a CommonIdV1 {
        each_event!(self, event => &event.data.queue_id)
    }

    fn actor(self) -> &'a CommonActorV1 {
        each_event!(self, event => &event.actor)
    }

    fn ts(self) -> &'a CommonTimestampV1 {
        each_event!(self, event => &event.ts)
    }

    fn task_creation(self) -> Option<TaskCreation<'a>> {
        match self {
            Self::TaskCreated(event) => Some(event.into()),
            Self::TaskCreatedV2(event) => Some(event.into()),
            Self::TaskCreatedV3(event) => Some(event.into()),
            Self::TaskCreatedV4(event) => Some(event.into()),
            _ => None,
        }
    }
}

/// One event of the `queue/<id>` stream, owned, as the storage hands it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueStreamEvent {
    Created(EventQueueCreatedV1),
    CreatedV2(EventQueueCreatedV2),
    Enabled(EventQueueEnabledV1),
    Stopped(EventQueueStoppedV1),
    ClockReanchored(EventQueueClockReanchoredV1),
    PolicyChanged(EventQueuePolicyChangedV1),
    TaskCreated(EventQueueTaskCreatedV1),
    TaskCreatedV2(EventQueueTaskCreatedV2),
    TaskCreatedV3(EventQueueTaskCreatedV3),
    TaskCreatedV4(EventQueueTaskCreatedV4),
    TaskStateChanged(EventQueueTaskStateChangedV1),
    TaskStateChangedV2(EventQueueTaskStateChangedV2),
}

/// Why an envelope is not an event of the queue stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueEventRefusal {
    /// The type is not one of the queue event versions this core reads.
    UnknownType,
    /// The envelope breaks the contract of its type.
    Invalid,
}

impl QueueStreamEvent {
    /// Reads an envelope of the queue stream into the contract of its type.
    pub fn from_envelope(envelope: &EventEnvelopeV1) -> Result<Self, QueueEventRefusal> {
        fn typed<T: serde::de::DeserializeOwned>(
            envelope: &EventEnvelopeV1,
        ) -> Result<T, QueueEventRefusal> {
            serde_json::to_value(envelope)
                .and_then(serde_json::from_value)
                .map_err(|_| QueueEventRefusal::Invalid)
        }
        Ok(match envelope.r#type.as_str() {
            "urn:ads:event:queue.created:1" => Self::Created(typed(envelope)?),
            "urn:ads:event:queue.created:2" => Self::CreatedV2(typed(envelope)?),
            "urn:ads:event:queue.enabled:1" => Self::Enabled(typed(envelope)?),
            "urn:ads:event:queue.stopped:1" => Self::Stopped(typed(envelope)?),
            "urn:ads:event:queue.clock_reanchored:1" => Self::ClockReanchored(typed(envelope)?),
            "urn:ads:event:queue.policy_changed:1" => Self::PolicyChanged(typed(envelope)?),
            "urn:ads:event:queue.task_created:1" => Self::TaskCreated(typed(envelope)?),
            "urn:ads:event:queue.task_created:2" => Self::TaskCreatedV2(typed(envelope)?),
            "urn:ads:event:queue.task_created:3" => Self::TaskCreatedV3(typed(envelope)?),
            "urn:ads:event:queue.task_created:4" => Self::TaskCreatedV4(typed(envelope)?),
            "urn:ads:event:queue.task_state_changed:1" => Self::TaskStateChanged(typed(envelope)?),
            "urn:ads:event:queue.task_state_changed:2" => {
                Self::TaskStateChangedV2(typed(envelope)?)
            }
            _ => return Err(QueueEventRefusal::UnknownType),
        })
    }

    /// The borrowed form `replay_task` and `replay_queue` read.
    pub fn as_log(&self) -> QueueLogEvent<'_> {
        match self {
            Self::Created(event) => QueueLogEvent::Created(event),
            Self::CreatedV2(event) => QueueLogEvent::CreatedV2(event),
            Self::Enabled(event) => QueueLogEvent::Enabled(event),
            Self::Stopped(event) => QueueLogEvent::Stopped(event),
            Self::ClockReanchored(event) => QueueLogEvent::ClockReanchored(event),
            Self::PolicyChanged(event) => QueueLogEvent::PolicyChanged(event),
            Self::TaskCreated(event) => QueueLogEvent::TaskCreated(event),
            Self::TaskCreatedV2(event) => QueueLogEvent::TaskCreatedV2(event),
            Self::TaskCreatedV3(event) => QueueLogEvent::TaskCreatedV3(event),
            Self::TaskCreatedV4(event) => QueueLogEvent::TaskCreatedV4(event),
            Self::TaskStateChanged(event) => QueueLogEvent::TaskStateChanged(event),
            Self::TaskStateChangedV2(event) => QueueLogEvent::TaskStateChangedV2(event),
        }
    }
}

/// Why a queue log does not rebuild a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayRefusal {
    /// The log does not start with `queue.created` at `seq` 1, or creates the queue twice.
    NotFromCreation,
    /// A `seq` is not the previous one plus one.
    SeqGap,
    /// An event names another queue or another stream.
    ForeignEvent,
    /// A device acted without belonging to the queue owner at that instant, or an owner action
    /// came from something that is not a device.
    NotOwnerDevice,
    /// The task entered `running` while the queue was stopped.
    QueueStopped,
    /// The change does not follow the current state or the attempt count.
    InvalidChange,
    /// The task was created twice or never.
    TaskCreation,
    /// The policy cannot be represented by the current core.
    InvalidPolicy,
    /// A task in the stream fails the admission policy in force when it was created.
    Admission(AdmissionRefusal),
}

/// State restored from the queue log for use after restart.
#[derive(Debug, PartialEq, Eq)]
pub struct ReplayedTask {
    pub state: CommonQueueTaskStateV1,
    pub attempts: u64,
    pub anchor: Option<ClockAnchor>,
    /// Absent for a legacy queue with no recorded policy.
    pub policy: Option<QueuePolicy>,
}

/// Rebuilds the state and the retry budget of one task from the whole stream of its queue,
/// from `queue.created` at `seq` 1, with no gap. `owner_at` reads the device registry at the
/// instant of each event: a device keeps the owner it had when it acted, and a revocation is a
/// dated mark, so it refuses later acts without erasing earlier ones.
///
/// The replay checks the coherence of the actors the log declares, not their authenticity:
/// the log is not signed per device, and a forged line is caught by the storage HMAC chain.
/// Every device actor belongs to the queue owner; creating, enabling and owner stops come from
/// such a device, and so do the manual requests of every task of the queue; a task enters
/// `running` only while the queue is enabled; and `review → done` is accepted by a device of the
/// owner. A task change names its channel exactly when a device made it. Agents and the daemon
/// may still cancel or fail tasks.
///
/// Replay starts with the policy recorded in queue.created:2. Owner policy events set the
/// policy for subsequent events. For queue.created:1, limits are unknown until the first
/// policy event; the replay still checks the history against policy-independent rules.
/// Each creation supplies its clock through the logged timestamp. Anchors come from the log.
/// The stream cannot establish uniqueness across queues.
pub fn replay_task<'a>(
    task_id: &CommonIdV1,
    log: impl IntoIterator<Item = QueueLogEvent<'a>>,
    owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
) -> Result<ReplayedTask, ReplayRefusal> {
    let replayed = replay(Some(task_id), log, owner_at)?;
    let task = replayed
        .tasks
        .into_iter()
        .next()
        .ok_or(ReplayRefusal::TaskCreation)?;
    Ok(ReplayedTask {
        state: task.state,
        attempts: task.attempts,
        anchor: replayed.anchor,
        policy: replayed.policy,
    })
}

/// Every task of a queue rebuilt from its stream, with the anchor and the policy in force.
#[derive(Debug, PartialEq, Eq)]
pub struct ReplayedQueue {
    /// One record per task, in creation order.
    pub tasks: Vec<RecordQueueTaskV3>,
    pub anchor: Option<ClockAnchor>,
    /// Absent for a legacy queue with no recorded policy.
    pub policy: Option<QueuePolicy>,
}

/// Rebuilds the record of every task of the queue in one pass over its stream, with the checks of
/// `replay_task` applied to every task, not only to one. A state change naming a task that was
/// never created in the stream is refused (`TaskCreation`), where `replay_task` skips the changes
/// of other tasks. A task created twice is refused (`TaskCreation`) by both, whichever task
/// `replay_task` follows.
///
/// `created_at` is the `ts` of the creation and `updated_at` the `ts` of the last event of the
/// task, or `created_at` when the clock went back to before the creation. Title and tool come
/// from `queue.task_created:4`; a task created by versions 1 to 3 did not record them, so its
/// record carries `null` in both instead of an invented value. No queue event records the branch
/// or the pull request, so the rebuilt record omits them.
pub fn replay_queue<'a>(
    log: impl IntoIterator<Item = QueueLogEvent<'a>>,
    owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
) -> Result<ReplayedQueue, ReplayRefusal> {
    let replayed = replay(None, log, owner_at)?;
    Ok(ReplayedQueue {
        tasks: replayed.tasks.iter().map(TaskProgress::record).collect(),
        anchor: replayed.anchor,
        policy: replayed.policy,
    })
}

/// A task as the replay follows it.
struct TaskProgress<'a> {
    creation: TaskCreation<'a>,
    /// Title and tool, recorded only by `queue.task_created:4`.
    details: Option<(&'a CommonQueueTaskTitleV1, &'a CommonIdV1)>,
    state: CommonQueueTaskStateV1,
    attempts: u64,
    updated_at: &'a CommonTimestampV1,
}

impl TaskProgress<'_> {
    fn record(&self) -> RecordQueueTaskV3 {
        let creation = self.creation;
        let origin = match creation.origin {
            TaskOrigin::Manual(request_id) => {
                CommonQueueTaskOriginV1::Manual(CommonQueueTaskOriginV1Manual {
                    request_id: request_id.clone(),
                })
            }
            TaskOrigin::Issue(number) => {
                CommonQueueTaskOriginV1::Issue(CommonQueueTaskOriginV1Issue {
                    r#ref: CommonQueueTaskOriginV1IssueRef::new(number)
                        .expect("every creation version keeps the issue positive"),
                })
            }
            TaskOrigin::BugRule(rule_id, target_id) => {
                CommonQueueTaskOriginV1::BugRule(CommonQueueTaskOriginV1BugRule {
                    rule_id: rule_id.clone(),
                    target_id: target_id.clone(),
                })
            }
        };
        RecordQueueTaskV3 {
            version: RecordQueueTaskV3Version,
            id: creation.task_id.clone(),
            queue_id: creation.queue_id.clone(),
            origin,
            title: self.details.map(|(title, _)| title.clone()),
            // Older versions repeat the rules of the common alias type (`tools/corpus.test.mjs`).
            workspace_alias: CommonWorkdirAliasV1::new(creation.workspace_alias)
                .expect("every creation version keeps the alias rules"),
            tool: self.details.map(|(_, tool)| tool.clone()),
            state: self.state,
            attempts: RecordQueueTaskV3Attempts::new(self.attempts)
                .expect("the replay keeps attempts within the ceiling"),
            attempt_limit: RecordQueueTaskV3AttemptLimit::new(creation.attempt_limit)
                .expect("every creation version keeps the ceiling from 1 to 10"),
            created_at: creation.ts.clone(),
            // A clock set back after the creation never dates the update before it.
            updated_at: if instant(self.updated_at) < instant(creation.ts) {
                creation.ts.clone()
            } else {
                self.updated_at.clone()
            },
            branch: None,
            pull_request: None,
        }
    }
}

struct Replayed<'a> {
    tasks: Vec<TaskProgress<'a>>,
    anchor: Option<ClockAnchor>,
    policy: Option<QueuePolicy>,
}

/// The one replay engine: follows `only` when given, else every task of the stream.
fn replay<'a>(
    only: Option<&CommonIdV1>,
    log: impl IntoIterator<Item = QueueLogEvent<'a>>,
    owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
) -> Result<Replayed<'a>, ReplayRefusal> {
    let mut log = log.into_iter();
    let first = log.next().ok_or(ReplayRefusal::NotFromCreation)?;
    if first.seq() != 1 {
        return Err(ReplayRefusal::NotFromCreation);
    }
    let (owner, initial_policy) = match first {
        QueueLogEvent::Created(created) => (&created.data.owner_id, None),
        QueueLogEvent::CreatedV2(created) => (&created.data.owner_id, Some(&created.data.policy)),
        _ => return Err(ReplayRefusal::NotFromCreation),
    };
    let queue_id = first.queue_id();
    let stream = format!("queue/{}", queue_id.as_str());
    let owner_device = |event: QueueLogEvent<'_>| {
        check_owner_device(event.actor(), event.ts(), owner, &owner_at).is_ok()
    };
    if first.stream() != stream {
        return Err(ReplayRefusal::ForeignEvent);
    }
    if !owner_device(first) {
        return Err(ReplayRefusal::NotOwnerDevice);
    }
    let mut queue_state = Stopped;
    let mut policy = initial_policy
        .map(|policy| QueuePolicy::from_event(policy).ok_or(ReplayRefusal::InvalidPolicy))
        .transpose()?;
    let followed = |task_id: &CommonIdV1| only.is_none_or(|only| only == task_id);
    let mut tasks: Vec<TaskProgress<'a>> = Vec::new();
    let mut index: HashMap<&'a str, usize> = HashMap::new();
    // Every task created in the stream, followed or not, so a second creation is refused alike.
    let mut created_ids: HashSet<&'a str> = HashSet::new();
    let mut admissions = Vec::new();
    let mut anchor = None;
    for (seq, event) in (2..).zip(log) {
        if event.seq() != seq {
            return Err(ReplayRefusal::SeqGap);
        }
        if event.queue_id() != queue_id || event.stream() != stream {
            return Err(ReplayRefusal::ForeignEvent);
        }
        if event.actor().kind == CommonActorV1Kind::Device && !owner_device(event) {
            return Err(ReplayRefusal::NotOwnerDevice);
        }
        match event {
            QueueLogEvent::Created(_) | QueueLogEvent::CreatedV2(_) => {
                return Err(ReplayRefusal::NotFromCreation);
            }
            QueueLogEvent::Enabled(enabled) => {
                if enabled.data.owner_id != *owner || !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                queue_state = Enabled;
            }
            QueueLogEvent::Stopped(stopped) => {
                if stopped.data.reason == EventQueueStoppedV1DataReason::Owner
                    && (stopped.data.owner_id.as_ref() != Some(owner) || !owner_device(event))
                {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                queue_state = Stopped;
            }
            QueueLogEvent::ClockReanchored(reanchored) => {
                if reanchored.data.owner_id != *owner || !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                anchor = Some(ClockAnchor::from_event(reanchored));
            }
            QueueLogEvent::PolicyChanged(changed) => {
                if changed.data.owner_id != *owner || !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                policy = Some(
                    QueuePolicy::from_event(&changed.data.policy)
                        .ok_or(ReplayRefusal::InvalidPolicy)?,
                );
            }
            QueueLogEvent::TaskCreated(_)
            | QueueLogEvent::TaskCreatedV2(_)
            | QueueLogEvent::TaskCreatedV3(_)
            | QueueLogEvent::TaskCreatedV4(_) => {
                let admitted = event.task_creation().expect("task creation event");
                if admitted.origin.is_manual() && !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                if !created_ids.insert(admitted.task_id.as_str()) {
                    return Err(ReplayRefusal::TaskCreation);
                }
                if let Some(policy) = policy.as_ref() {
                    let inputs = AdmissionInputs {
                        policy,
                        now: admitted.ts,
                        anchor: anchor.as_ref(),
                    };
                    check_admission(admitted, admissions.iter().copied(), owner, inputs)
                        .map_err(ReplayRefusal::Admission)?;
                } else {
                    if epoch_seconds(admitted.ts.as_str()).is_none() {
                        return Err(ReplayRefusal::Admission(
                            AdmissionRefusal::UnreadableInstant,
                        ));
                    }
                    if admissions
                        .iter()
                        .any(|prior| admitted.repeats_origin(*prior))
                    {
                        return Err(ReplayRefusal::Admission(AdmissionRefusal::OriginRepeated));
                    }
                }
                admissions.push(admitted);
                if !followed(admitted.task_id) {
                    continue;
                }
                let details = match event {
                    QueueLogEvent::TaskCreatedV4(created) => {
                        Some((&created.data.title, &created.data.tool))
                    }
                    _ => None,
                };
                index.insert(admitted.task_id.as_str(), tasks.len());
                tasks.push(TaskProgress {
                    creation: admitted,
                    details,
                    state: Queued,
                    attempts: 0,
                    updated_at: admitted.ts,
                });
            }
            QueueLogEvent::TaskStateChanged(_) | QueueLogEvent::TaskStateChangedV2(_) => {
                let (changed_task, from, to, recorded_attempts) = match event {
                    QueueLogEvent::TaskStateChanged(change) => {
                        let data = &change.data;
                        (&data.task_id, data.from, data.to, data.attempts.get())
                    }
                    QueueLogEvent::TaskStateChangedV2(change) => {
                        let data = &change.data;
                        (&data.task_id, data.from, data.to, data.attempts.get())
                    }
                    _ => unreachable!("task state change"),
                };
                if !followed(changed_task) {
                    continue;
                }
                let position = *index
                    .get(changed_task.as_str())
                    .ok_or(ReplayRefusal::TaskCreation)?;
                let task = &mut tasks[position];
                let limit = u64::from(task.creation.attempt_limit);
                let (state, attempts) = (&mut task.state, &mut task.attempts);
                if from != *state {
                    return Err(ReplayRefusal::InvalidChange);
                }
                if to == Running {
                    if queue_state != Enabled {
                        return Err(ReplayRefusal::QueueStopped);
                    }
                    *attempts += 1;
                }
                if (*state, to) == (Review, Done) && !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                if *attempts > limit
                    || recorded_attempts != *attempts
                    || (*state == Failed && to == Queued && *attempts == limit)
                {
                    return Err(ReplayRefusal::InvalidChange);
                }
                *state = to;
                task.updated_at = event.ts();
            }
        }
    }
    if only.is_some() && tasks.is_empty() {
        return Err(ReplayRefusal::TaskCreation);
    }
    Ok(Replayed {
        tasks,
        anchor,
        policy,
    })
}

/// `ts` as (date and time to the second, microseconds), in the order of the instants: the text
/// alone misorders `...:05Z` after `...:05.5Z`.
fn instant(ts: &CommonTimestampV1) -> (&str, u32) {
    let text = ts.as_str().strip_suffix('Z').unwrap_or(ts.as_str());
    let (seconds, fraction) = text.split_once('.').unwrap_or((text, ""));
    let micros = fraction
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(6)
        .fold(0, |micros, digit| micros * 10 + u32::from(digit - b'0'));
    (seconds, micros)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TaskOrigin<'a> {
    Manual(&'a CommonIdV1),
    Issue(u64),
    BugRule(&'a CommonIdV1, &'a CommonIdV1),
}

impl<'a> TaskOrigin<'a> {
    fn is_manual(self) -> bool {
        matches!(self, Self::Manual(_))
    }

    fn bug_rule(self) -> Option<&'a CommonIdV1> {
        match self {
            Self::BugRule(rule, _) => Some(rule),
            _ => None,
        }
    }
}

/// A borrowed task creation from any version of the queue contract.
#[derive(Clone, Copy)]
pub struct TaskCreation<'a> {
    queue_id: &'a CommonIdV1,
    task_id: &'a CommonIdV1,
    attempt_limit: u16,
    origin: TaskOrigin<'a>,
    workspace_alias: &'a str,
    ts: &'a CommonTimestampV1,
    seq: u64,
    actor: &'a CommonActorV1,
}

macro_rules! task_creation_from {
    ($event:ident, $origin:ident) => {
        impl<'a> From<&'a $event> for TaskCreation<'a> {
            fn from(event: &'a $event) -> Self {
                let data = &event.data;
                let origin = match &data.origin {
                    $origin::Manual(manual) => TaskOrigin::Manual(&manual.request_id),
                    $origin::Issue(issue) => TaskOrigin::Issue(issue.r#ref.get()),
                    $origin::BugRule(bug) => TaskOrigin::BugRule(&bug.rule_id, &bug.target_id),
                };
                Self {
                    queue_id: &data.queue_id,
                    task_id: &data.task_id,
                    attempt_limit: data.attempt_limit.get(),
                    origin,
                    workspace_alias: data.workspace_alias.as_str(),
                    ts: &event.ts,
                    seq: event.seq.get(),
                    actor: &event.actor,
                }
            }
        }
    };
}

task_creation_from!(EventQueueTaskCreatedV1, EventQueueTaskCreatedV1DataOrigin);
task_creation_from!(EventQueueTaskCreatedV2, EventQueueTaskCreatedV2DataOrigin);
task_creation_from!(EventQueueTaskCreatedV3, CommonQueueTaskOriginV1);
task_creation_from!(EventQueueTaskCreatedV4, CommonQueueTaskOriginV1);

/// Why the admission refuses a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRefusal {
    /// The attempt ceiling of the task is above the policy.
    AttemptCeiling,
    /// A manual request was not created by a device of the queue owner.
    ManualNotOwnerDevice,
    /// The clock anchor belongs to another queue or another owner.
    ForeignAnchor,
    /// The clock anchor covers the candidate itself or later events, so it would move
    /// admissions made after the owner acted.
    AnchorAfterCandidate,
    /// The clock anchor is more than `clock_skew_seconds` ahead of the core clock.
    AnchorAheadOfClock,
    /// An instant could not be read.
    UnreadableInstant,
    /// The stamp is more than `clock_skew_seconds` ahead of the core clock.
    AheadOfClock,
    /// The stamp is more than `clock_rewind_seconds` behind an earlier admission of the queue,
    /// usually because the core clock ran ahead and was corrected; the owner re-anchors it.
    BehindLastAdmission,
    /// The task id was already used in some queue.
    TaskIdUsed,
    /// The origin was already admitted in this queue.
    OriginRepeated,
    /// The queue reached `admission_limit`.
    AdmissionLimit,
    /// The window of manual requests, or of issues and bug rules, is full.
    WindowFull,
    /// The window of this bug rule is full.
    BugRuleWindowFull,
}

/// Owner action that re-anchors the admission clock of a queue after the core clock ran ahead.
/// Earlier admissions of the queue, up to `through_seq`, stamped after `at` and still ahead of
/// the core clock count as admitted at `at`: they keep their origin reserved and fill the window
/// that follows the anchor. The admission refuses an anchor that covers the candidate or a later
/// `seq`, or whose `at` is ahead of the core clock.
///
/// Built from an authenticated owner action or restored by replay after validating the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockAnchor {
    queue_id: CommonIdV1,
    owner_id: CommonIdV1,
    device_id: CommonIdV1,
    via: Channel,
    at: CommonTimestampV1,
    through_seq: u64,
}

impl ClockAnchor {
    fn from_event(event: &EventQueueClockReanchoredV1) -> Self {
        Self {
            queue_id: event.data.queue_id.clone(),
            owner_id: event.data.owner_id.clone(),
            device_id: event.actor.id.clone(),
            via: match event.data.via {
                EventQueueClockReanchoredV1DataVia::LocalUi => Channel::LocalUi,
                EventQueueClockReanchoredV1DataVia::Tray => Channel::Tray,
                EventQueueClockReanchoredV1DataVia::RemoteDevice => Channel::RemoteDevice,
            },
            at: event.ts.clone(),
            through_seq: event.seq.get() - 1,
        }
    }

    /// Payload to append at `through_seq + 1`, with `at` as the envelope timestamp.
    pub fn event_data(&self) -> EventQueueClockReanchoredV1Data {
        EventQueueClockReanchoredV1Data {
            queue_id: self.queue_id.clone(),
            owner_id: self.owner_id.clone(),
            via: match self.via {
                Channel::LocalUi => EventQueueClockReanchoredV1DataVia::LocalUi,
                Channel::Tray => EventQueueClockReanchoredV1DataVia::Tray,
                Channel::RemoteDevice => EventQueueClockReanchoredV1DataVia::RemoteDevice,
            },
        }
    }

    pub fn queue_id(&self) -> &CommonIdV1 {
        &self.queue_id
    }

    pub fn owner_id(&self) -> &CommonIdV1 {
        &self.owner_id
    }

    pub fn device_id(&self) -> &CommonIdV1 {
        &self.device_id
    }

    pub fn via(&self) -> Channel {
        self.via
    }

    /// Core clock when the owner acted.
    pub fn at(&self) -> &CommonTimestampV1 {
        &self.at
    }

    /// Last `seq` of the queue stream when the owner acted.
    pub fn through_seq(&self) -> u64 {
        self.through_seq
    }
}

/// Builds the anchor from the owner action. `now` is the core clock when the owner acts, with
/// the same contract as `created.ts`; `through_seq` is the `seq` the owner acted on, and
/// `stream_last_seq` the last `seq` of the queue stream read from storage, which must be equal.
/// The producer conditionally appends the event at the next sequence, using the owner device as
/// actor and `now` as `ts`. The anchor takes effect only after the append succeeds.
pub fn reanchor_admission_clock(
    queue: &RecordTaskQueueV1,
    device: Option<&AuthenticatedDevice>,
    now: &CommonTimestampV1,
    through_seq: u64,
    stream_last_seq: u64,
) -> Result<ClockAnchor, QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    if device.owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
    }
    if through_seq != stream_last_seq {
        return Err(QueueRefusal::StaleSeq);
    }
    Ok(ClockAnchor {
        queue_id: queue.id.clone(),
        owner_id: device.owner_id.clone(),
        device_id: device.device_id.clone(),
        via: device.via,
        at: now.clone(),
        through_seq,
    })
}

/// Authorizes a policy change against the last sequence read from storage. Like enabling,
/// it accepts only `local_ui` and `tray`. The caller appends conditionally at the next
/// sequence using the authenticated device as actor and activates the policy only after
/// the append succeeds.
pub fn change_queue_policy(
    queue: &RecordTaskQueueV1,
    device: Option<&AuthenticatedDevice>,
    policy: &QueuePolicy,
    expected_seq: u64,
    stream_last_seq: u64,
) -> Result<EventQueuePolicyChangedV1Data, QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    if device.owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
    }
    let via = match device.via {
        Channel::LocalUi => EventQueuePolicyChangedV1DataVia::LocalUi,
        Channel::Tray => EventQueuePolicyChangedV1DataVia::Tray,
        Channel::RemoteDevice => return Err(QueueRefusal::RemotePolicyChange),
    };
    if expected_seq != stream_last_seq {
        return Err(QueueRefusal::StaleSeq);
    }
    Ok(EventQueuePolicyChangedV1Data {
        queue_id: queue.id.clone(),
        owner_id: device.owner_id.clone(),
        via,
        policy: policy.event_data(),
    })
}

/// What the producer knows when it decides an admission, besides the candidate and the history.
pub struct AdmissionContext<'a> {
    pub policy: &'a QueuePolicy,
    /// Core clock when the producer decides.
    pub now: &'a CommonTimestampV1,
    /// Owner of the queue the candidate names.
    pub queue_owner: &'a CommonIdV1,
    /// Device that asked for the task, required for a manual request.
    pub author: Option<&'a AuthenticatedDevice>,
    /// Latest clock anchor of the queue, if the owner re-anchored it.
    pub anchor: Option<&'a ClockAnchor>,
}

struct AdmissionInputs<'a> {
    /// Policy in force at the decision.
    pub policy: &'a QueuePolicy,
    /// Core clock at the decision.
    pub now: &'a CommonTimestampV1,
    /// Latest owner anchor at the decision, if any.
    pub anchor: Option<&'a ClockAnchor>,
}

fn authored_by_owner(created: TaskCreation<'_>, context: &AdmissionContext<'_>) -> bool {
    context.author.is_some_and(|author| {
        author.owner_id == *context.queue_owner
            && created.actor.kind == CommonActorV1Kind::Device
            && created.actor.id == author.device_id
    })
}

/// Previously admitted origins remain reserved after termination and replay. Task ids are
/// unique across queues, and the attempt ceiling of a new task follows the configured policy.
/// A manual request is created by a device of the queue owner, with `actor.id` equal to its id.
///
/// `created.ts` must come from the core clock, never from the request, and `now` is that clock
/// when the producer decides. A candidate stamped more than `clock_skew_seconds` after `now`, or
/// more than `clock_rewind_seconds` before an earlier admission of the queue, is refused, so a
/// stamp ahead of the core clock does not empty the window. If the core clock itself runs ahead,
/// the window can empty. After correction, `BehindLastAdmission` takes precedence over count
/// limits until the clock returns within tolerance or the owner re-anchors it.
///
/// In any `window_seconds` ending at the candidate `ts`, the queue admits at most
/// `manual_window_admissions` manual requests and `window_admissions` tasks from issues and bug
/// rules, and at most `bug_rule_window_admissions` from the same bug rule. Automatic origins
/// therefore never use up the room of the owner's requests. The window limits clear as time
/// passes; the lifetime limit clears only when the owner raises it.
pub fn can_admit_task<'a>(
    created: &EventQueueTaskCreatedV1,
    history: impl IntoIterator<Item = &'a EventQueueTaskCreatedV1>,
    context: &AdmissionContext<'_>,
) -> Result<(), AdmissionRefusal> {
    can_admit_creation(created.into(), history.into_iter().map(Into::into), context)
}

/// Checks admission of any event version against a history that can mix versions.
pub fn can_admit_creation<'a>(
    created: TaskCreation<'_>,
    history: impl IntoIterator<Item = TaskCreation<'a>>,
    context: &AdmissionContext<'_>,
) -> Result<(), AdmissionRefusal> {
    if created.origin.is_manual() && !authored_by_owner(created, context) {
        return Err(AdmissionRefusal::ManualNotOwnerDevice);
    }
    check_admission(
        created,
        history,
        context.queue_owner,
        AdmissionInputs {
            policy: context.policy,
            now: context.now,
            anchor: context.anchor,
        },
    )
}

impl TaskCreation<'_> {
    fn repeats_origin(self, prior: TaskCreation<'_>) -> bool {
        prior.queue_id == self.queue_id
            && prior.workspace_alias == self.workspace_alias
            && prior.origin == self.origin
    }
}

fn check_admission<'a>(
    created: TaskCreation<'_>,
    history: impl IntoIterator<Item = TaskCreation<'a>>,
    queue_owner: &CommonIdV1,
    inputs: AdmissionInputs<'_>,
) -> Result<(), AdmissionRefusal> {
    use AdmissionRefusal as Refusal;
    let policy = inputs.policy;
    if created.attempt_limit > policy.attempt_limit.get() {
        return Err(Refusal::AttemptCeiling);
    }
    let manual = created.origin.is_manual();
    let (Some(stamp), Some(clock)) = (
        epoch_seconds(created.ts.as_str()),
        epoch_seconds(inputs.now.as_str()),
    ) else {
        return Err(Refusal::UnreadableInstant);
    };
    let seconds = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
    let latest_now = clock.saturating_add(seconds(policy.clock_skew_seconds));
    if stamp > latest_now {
        return Err(Refusal::AheadOfClock);
    }
    let anchor = match inputs.anchor {
        Some(anchor) if &anchor.queue_id != created.queue_id || anchor.owner_id != *queue_owner => {
            return Err(Refusal::ForeignAnchor);
        }
        Some(anchor) if anchor.through_seq >= created.seq => {
            return Err(Refusal::AnchorAfterCandidate);
        }
        Some(anchor) => {
            let at = epoch_seconds(anchor.at.as_str()).ok_or(Refusal::UnreadableInstant)?;
            if at > latest_now {
                return Err(Refusal::AnchorAheadOfClock);
            }
            Some((anchor.through_seq, at))
        }
        None => None,
    };
    let latest_prior = stamp.saturating_add(seconds(policy.clock_rewind_seconds));
    let window = seconds(policy.window_seconds.get());
    let rule = created.origin.bug_rule();
    let (class_limit, rule_limit) = if manual {
        (policy.manual_window_admissions.get(), usize::MAX)
    } else {
        (
            policy.window_admissions.get(),
            policy.bug_rule_window_admissions.get(),
        )
    };
    let (mut count, mut same_class, mut same_rule) = (0, 0, 0);
    for prior in history {
        if prior.task_id == created.task_id {
            return Err(Refusal::TaskIdUsed);
        }
        if prior.queue_id != created.queue_id {
            continue;
        }
        if created.repeats_origin(prior) {
            return Err(Refusal::OriginRepeated);
        }
        // The anchor moves only covered creations still ahead of the clock, so an old anchor
        // never takes an ordinary creation out of the window.
        let then = epoch_seconds(prior.ts.as_str()).map(|then| match anchor {
            Some((through, at)) if prior.seq <= through && then > latest_now => then.min(at),
            _ => then,
        });
        if then.is_some_and(|then| then > latest_prior) {
            return Err(Refusal::BehindLastAdmission);
        }
        count += 1;
        // A prior stamped after the candidate, within the rewind, is recent too.
        let recent = then.is_none_or(|then| stamp.saturating_sub(then) < window);
        if recent && prior.origin.is_manual() == manual {
            same_class += 1;
            if rule.is_some() && prior.origin.bug_rule() == rule {
                same_rule += 1;
            }
        }
    }
    if count >= policy.admission_limit.get() {
        return Err(Refusal::AdmissionLimit);
    }
    if same_class >= class_limit {
        return Err(Refusal::WindowFull);
    }
    if same_rule >= rule_limit {
        return Err(Refusal::BugRuleWindowFull);
    }
    Ok(())
}

/// Returns the attempt count after an allowed transition in the current queue.
pub fn next_attempt_count<'a>(
    queue: &RecordTaskQueueV1,
    task: impl Into<TaskSnapshot<'a>>,
    next: CommonQueueTaskStateV1,
    device: Option<&AuthenticatedDevice>,
) -> Option<RecordQueueTaskV2Attempts> {
    let task = task.into();
    if task.queue_id != &queue.id {
        return None;
    }
    if next == Cancelled && matches!(task.state, Queued | Running | Review | Failed) {
        return RecordQueueTaskV2Attempts::new(task.attempts).ok();
    }

    let attempts = task.attempts;
    let limit = task.limit;
    if attempts > limit || (attempts == 0 && task.state != Queued) {
        return None;
    }

    match (task.state, next) {
        (Queued, Running) if queue.state == Enabled && attempts < limit => {
            RecordQueueTaskV2Attempts::new(attempts + 1).ok()
        }
        (Failed, Queued) if attempts < limit => RecordQueueTaskV2Attempts::new(task.attempts).ok(),
        (Review, Done) if device.is_some_and(|device| device.owner_id == queue.owner_id) => {
            RecordQueueTaskV2Attempts::new(task.attempts).ok()
        }
        (Running, Review | Failed) | (Review, Failed) => {
            RecordQueueTaskV2Attempts::new(task.attempts).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
