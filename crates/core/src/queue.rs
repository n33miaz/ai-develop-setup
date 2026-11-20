//! Authorization and lifecycle policy for the task queue.

use std::num::{NonZeroU64, NonZeroUsize};

use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonIdV1, CommonQueueTaskStateV1, CommonTimestampV1,
    EventQueueCreatedV1, EventQueueCreatedV1DataVia, EventQueueEnabledV1, EventQueueEnabledV1Data,
    EventQueueEnabledV1DataVia, EventQueueStoppedV1, EventQueueStoppedV1Data,
    EventQueueStoppedV1DataReason, EventQueueStoppedV1DataVia, EventQueueTaskCreatedV1,
    EventQueueTaskCreatedV1DataOrigin, EventQueueTaskStateChangedV1,
    EventQueueTaskStateChangedV1Data, EventQueueTaskStateChangedV1DataAttempts,
    EventQueueTaskStateChangedV1DataFields, EventQueueTaskStateChangedV1DataVia, RecordQueueTaskV1,
    RecordQueueTaskV1Attempts, RecordTaskQueueV1, RecordTaskQueueV1State, RecordTaskQueueV1Version,
};

use CommonQueueTaskStateV1::{Cancelled, Done, Failed, Queued, Review, Running};
use RecordTaskQueueV1State::{Enabled, Stopped};

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/queue.toml");

/// Limits of the queue. Keys missing from an older file take the values of `defaults/queue.toml`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuePolicy {
    pub attempt_limit: ads_contracts::RecordQueueTaskV1AttemptLimit,
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

/// Channel through which a device acted, independent of the event that records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    LocalUi,
    Tray,
    RemoteDevice,
}

impl Channel {
    pub fn created_via(self) -> EventQueueCreatedV1DataVia {
        match self {
            Self::LocalUi => EventQueueCreatedV1DataVia::LocalUi,
            Self::Tray => EventQueueCreatedV1DataVia::Tray,
            Self::RemoteDevice => EventQueueCreatedV1DataVia::RemoteDevice,
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

    pub fn task_via(self) -> EventQueueTaskStateChangedV1DataVia {
        match self {
            Self::LocalUi => EventQueueTaskStateChangedV1DataVia::LocalUi,
            Self::Tray => EventQueueTaskStateChangedV1DataVia::Tray,
            Self::RemoteDevice => EventQueueTaskStateChangedV1DataVia::RemoteDevice,
        }
    }
}

/// Device, the owner it is registered to and channel, established by the authentication boundary.
///
/// There is deliberately no public constructor. Any public function of this crate can be called
/// by every crate that depends on it, and the dependency matrix keeps the authentication layer
/// out of this crate, so no constructor here could tell the authentication layer from a caller
/// that copies a device id out of a request. The constructor arrives together with that layer.
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
    Unchanged,
}

pub fn new_stopped(id: CommonIdV1, owner: &AuthenticatedDevice) -> RecordTaskQueueV1 {
    RecordTaskQueueV1 {
        version: RecordTaskQueueV1Version,
        id,
        owner_id: owner.owner_id.clone(),
        state: Stopped,
    }
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

/// Whether the terminal host still runs the process of a task when the core starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProcess {
    Kept,
    Lost,
}

/// What the start does with a task, with the data of the change to append, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunningAfterRestart {
    /// No change: the task is not running, or the host kept its process and the attempt goes on.
    Keep,
    /// `running → failed` with the same count, because the interrupted attempt was already counted.
    Fail(EventQueueTaskStateChangedV1Data),
    /// `running → cancelled` with the recorded count, for a running task whose count is
    /// inconsistent; a process the host kept is ended by the caller.
    Cancel(EventQueueTaskStateChangedV1Data),
}

/// Decides, on start, the fate of a task whose last recorded state is `running`.
pub fn restore_running_task(task: &RecordQueueTaskV1, host: HostProcess) -> RunningAfterRestart {
    if task.state != Running {
        return RunningAfterRestart::Keep;
    }
    let attempts = task.attempts.get();
    let consistent = attempts >= 1 && attempts <= task.attempt_limit.get().into();
    let to = match (consistent, host) {
        (true, HostProcess::Kept) => return RunningAfterRestart::Keep,
        (true, HostProcess::Lost) => Failed,
        (false, _) => Cancelled,
    };
    let data = EventQueueTaskStateChangedV1Data::new(EventQueueTaskStateChangedV1DataFields {
        queue_id: task.queue_id.clone(),
        task_id: task.id.clone(),
        from: Running,
        to,
        attempts: EventQueueTaskStateChangedV1DataAttempts::new(attempts)
            .expect("a record count fits the event count"),
        via: None,
    })
    .expect("running to failed or cancelled is an allowed change");
    if to == Failed {
        RunningAfterRestart::Fail(data)
    } else {
        RunningAfterRestart::Cancel(data)
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
    if device.owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
    }
    if next == Enabled && device.via.enabled_via().is_none() {
        return Err(QueueRefusal::RemoteEnable);
    }
    if queue.state == next {
        return Err(QueueRefusal::Unchanged);
    }
    let event = match next {
        Enabled => QueueStateEvent::Enabled(EventQueueEnabledV1Data {
            queue_id: queue.id.clone(),
            owner_id: device.owner_id.clone(),
            via: device.via.enabled_via().ok_or(QueueRefusal::RemoteEnable)?,
        }),
        Stopped => QueueStateEvent::Stopped(EventQueueStoppedV1Data {
            queue_id: queue.id.clone(),
            reason: EventQueueStoppedV1DataReason::Owner,
            owner_id: Some(device.owner_id.clone()),
            via: Some(device.via.stopped_via()),
        }),
    };
    let mut updated = queue.clone();
    updated.state = next;
    Ok((updated, event, device))
}

/// One event of the `queue/<id>` stream, borrowed for replay.
#[derive(Debug, Clone, Copy)]
pub enum QueueLogEvent<'a> {
    Created(&'a EventQueueCreatedV1),
    Enabled(&'a EventQueueEnabledV1),
    Stopped(&'a EventQueueStoppedV1),
    TaskCreated(&'a EventQueueTaskCreatedV1),
    TaskStateChanged(&'a EventQueueTaskStateChangedV1),
}

macro_rules! each_event {
    ($event:expr, $name:ident => $body:expr) => {
        match $event {
            QueueLogEvent::Created($name) => $body,
            QueueLogEvent::Enabled($name) => $body,
            QueueLogEvent::Stopped($name) => $body,
            QueueLogEvent::TaskCreated($name) => $body,
            QueueLogEvent::TaskStateChanged($name) => $body,
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
/// owner. Agents and the daemon may still cancel or fail tasks.
///
/// The replay does not run the admission again: neither the policy in force nor the clock anchor
/// of each creation is in the log, so checking old creations against the current policy would
/// refuse a legitimate history once the owner lowers a limit.
pub fn replay_task<'a>(
    task_id: &CommonIdV1,
    log: impl IntoIterator<Item = QueueLogEvent<'a>>,
    owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
) -> Result<(CommonQueueTaskStateV1, u64), ReplayRefusal> {
    let mut log = log.into_iter();
    let Some(QueueLogEvent::Created(created)) = log.next() else {
        return Err(ReplayRefusal::NotFromCreation);
    };
    if created.seq.get() != 1 {
        return Err(ReplayRefusal::NotFromCreation);
    }
    let queue_id = &created.data.queue_id;
    let owner = &created.data.owner_id;
    let stream = format!("queue/{}", queue_id.as_str());
    let owner_device = |event: QueueLogEvent<'_>| {
        let actor = event.actor();
        actor.kind == CommonActorV1Kind::Device
            && owner_at(&actor.id, event.ts()).as_ref() == Some(owner)
    };
    let first = QueueLogEvent::Created(created);
    if first.stream() != stream {
        return Err(ReplayRefusal::ForeignEvent);
    }
    if !owner_device(first) {
        return Err(ReplayRefusal::NotOwnerDevice);
    }
    let mut queue_state = Stopped;
    // (state, attempts, ceiling) once the task is created.
    let mut task: Option<(CommonQueueTaskStateV1, u64, u64)> = None;
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
            QueueLogEvent::Created(_) => return Err(ReplayRefusal::NotFromCreation),
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
            QueueLogEvent::TaskCreated(admitted) => {
                if is_manual(&admitted.data.origin) && !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                if admitted.data.task_id != *task_id {
                    continue;
                }
                if task.is_some() {
                    return Err(ReplayRefusal::TaskCreation);
                }
                task = Some((Queued, 0, admitted.data.attempt_limit.get().into()));
            }
            QueueLogEvent::TaskStateChanged(change) => {
                if change.data.task_id != *task_id {
                    continue;
                }
                let (state, attempts, limit) = task.as_mut().ok_or(ReplayRefusal::TaskCreation)?;
                if change.data.from != *state {
                    return Err(ReplayRefusal::InvalidChange);
                }
                if change.data.to == Running {
                    if queue_state != Enabled {
                        return Err(ReplayRefusal::QueueStopped);
                    }
                    *attempts += 1;
                }
                if (*state, change.data.to) == (Review, Done) && !owner_device(event) {
                    return Err(ReplayRefusal::NotOwnerDevice);
                }
                if *attempts > *limit
                    || change.data.attempts.get() != *attempts
                    || (*state == Failed && change.data.to == Queued && *attempts == *limit)
                {
                    return Err(ReplayRefusal::InvalidChange);
                }
                *state = change.data.to;
            }
        }
    }
    task.map(|(state, attempts, _)| (state, attempts))
        .ok_or(ReplayRefusal::TaskCreation)
}

/// Seconds since 1970-01-01T00:00:00Z, ignoring the fraction.
fn epoch_seconds(ts: &CommonTimestampV1) -> Option<i64> {
    let text = ts.as_str();
    let number = |from: usize, to: usize| text.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    // Days from the civil date, counting years from March so that February is last.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn bug_rule(origin: &EventQueueTaskCreatedV1DataOrigin) -> Option<&CommonIdV1> {
    match origin {
        EventQueueTaskCreatedV1DataOrigin::BugRule(bug) => Some(&bug.rule_id),
        _ => None,
    }
}

fn is_manual(origin: &EventQueueTaskCreatedV1DataOrigin) -> bool {
    matches!(origin, EventQueueTaskCreatedV1DataOrigin::Manual(_))
}

/// Why the admission refuses a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRefusal {
    /// The attempt ceiling of the task is above the policy.
    AttemptCeiling,
    /// A manual request was not created by a device of the queue owner.
    ManualNotOwnerDevice,
    /// The clock anchor belongs to another queue or another owner.
    ForeignAnchor,
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
/// Earlier admissions of the queue, up to `through_seq`, stamped after `at` count as admitted at
/// `at`: they keep their origin reserved and fill the window that follows the anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockAnchor {
    pub queue_id: CommonIdV1,
    pub owner_id: CommonIdV1,
    pub device_id: CommonIdV1,
    pub via: Channel,
    /// Core clock when the owner acted.
    pub at: CommonTimestampV1,
    /// Last `seq` of the queue stream when the owner acted.
    pub through_seq: u64,
}

/// Builds the anchor from the owner action. Recording it needs an event the contracts do not
/// have yet, so until then the caller keeps it.
pub fn reanchor_admission_clock(
    queue: &RecordTaskQueueV1,
    device: Option<&AuthenticatedDevice>,
    now: &CommonTimestampV1,
    through_seq: u64,
) -> Result<ClockAnchor, QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    if device.owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
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

fn authored_by_owner(created: &EventQueueTaskCreatedV1, context: &AdmissionContext<'_>) -> bool {
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
/// the window empties once; once the clock is corrected, admissions stop until real time reaches
/// the last one or the owner re-anchors the clock with `reanchor_admission_clock`.
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
    use AdmissionRefusal as Refusal;
    let policy = context.policy;
    if created.data.attempt_limit.get() > policy.attempt_limit.get() {
        return Err(Refusal::AttemptCeiling);
    }
    let manual = is_manual(&created.data.origin);
    if manual && !authored_by_owner(created, context) {
        return Err(Refusal::ManualNotOwnerDevice);
    }
    let anchor = match context.anchor {
        Some(anchor)
            if anchor.queue_id != created.data.queue_id
                || anchor.owner_id != *context.queue_owner =>
        {
            return Err(Refusal::ForeignAnchor);
        }
        Some(anchor) => Some((
            anchor.through_seq,
            epoch_seconds(&anchor.at).ok_or(Refusal::UnreadableInstant)?,
        )),
        None => None,
    };
    let (Some(stamp), Some(clock)) = (epoch_seconds(&created.ts), epoch_seconds(context.now))
    else {
        return Err(Refusal::UnreadableInstant);
    };
    let seconds = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
    if stamp > clock.saturating_add(seconds(policy.clock_skew_seconds)) {
        return Err(Refusal::AheadOfClock);
    }
    let latest_prior = stamp.saturating_add(seconds(policy.clock_rewind_seconds));
    let window = seconds(policy.window_seconds.get());
    let rule = bug_rule(&created.data.origin);
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
        if prior.data.task_id == created.data.task_id {
            return Err(Refusal::TaskIdUsed);
        }
        if prior.data.queue_id != created.data.queue_id {
            continue;
        }
        if prior.data.workspace_alias == created.data.workspace_alias
            && prior.data.origin == created.data.origin
        {
            return Err(Refusal::OriginRepeated);
        }
        let then = epoch_seconds(&prior.ts).map(|then| match anchor {
            Some((through, at)) if prior.seq.get() <= through => then.min(at),
            _ => then,
        });
        if then.is_some_and(|then| then > latest_prior) {
            return Err(Refusal::BehindLastAdmission);
        }
        count += 1;
        // A prior stamped after the candidate, within the rewind, is recent too.
        let recent = then.is_none_or(|then| stamp.saturating_sub(then) < window);
        if recent && is_manual(&prior.data.origin) == manual {
            same_class += 1;
            if rule.is_some() && bug_rule(&prior.data.origin) == rule {
                same_rule += 1;
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
    }
    Ok(())
}

/// Returns the attempt count after an allowed transition in the current queue.
pub fn next_attempt_count(
    queue: &RecordTaskQueueV1,
    task: &RecordQueueTaskV1,
    next: CommonQueueTaskStateV1,
    device: Option<&AuthenticatedDevice>,
) -> Option<RecordQueueTaskV1Attempts> {
    if task.queue_id != queue.id {
        return None;
    }
    if next == Cancelled && matches!(task.state, Queued | Running | Review | Failed) {
        return Some(task.attempts);
    }

    let attempts = task.attempts.get();
    let limit = task.attempt_limit.get().into();
    if attempts > limit || (attempts == 0 && task.state != Queued) {
        return None;
    }

    match (task.state, next) {
        (Queued, Running) if queue.state == Enabled && attempts < limit => {
            RecordQueueTaskV1Attempts::new(attempts + 1).ok()
        }
        (Failed, Queued) if attempts < limit => Some(task.attempts),
        (Review, Done) if device.is_some_and(|device| device.owner_id == queue.owner_id) => {
            Some(task.attempts)
        }
        (Running, Review | Failed) | (Review, Failed) => Some(task.attempts),
        _ => None,
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
