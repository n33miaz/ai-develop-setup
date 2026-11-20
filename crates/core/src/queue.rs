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

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuePolicy {
    pub attempt_limit: ads_contracts::RecordQueueTaskV1AttemptLimit,
    pub admission_limit: NonZeroUsize,
    pub window_seconds: NonZeroU64,
    pub window_admissions: NonZeroUsize,
    pub bug_rule_window_admissions: NonZeroUsize,
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

/// On start, a running task whose process the host lost fails without a new attempt, because
/// the interrupted attempt was counted when the task entered `running`. A task whose process
/// the host kept stays running and finishes its attempt as usual. `None` means no change; a
/// running task with an inconsistent count is ended by cancelling it.
pub fn restore_running_task(
    task: &RecordQueueTaskV1,
    host: HostProcess,
) -> Option<EventQueueTaskStateChangedV1Data> {
    let attempts = task.attempts.get();
    if task.state != Running
        || host == HostProcess::Kept
        || attempts == 0
        || attempts > task.attempt_limit.get().into()
    {
        return None;
    }
    EventQueueTaskStateChangedV1Data::new(EventQueueTaskStateChangedV1DataFields {
        queue_id: task.queue_id.clone(),
        task_id: task.id.clone(),
        from: Running,
        to: Failed,
        attempts: EventQueueTaskStateChangedV1DataAttempts::new(attempts).ok()?,
        via: None,
    })
    .ok()
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

impl QueueLogEvent<'_> {
    fn seq(&self) -> u64 {
        match self {
            Self::Created(event) => event.seq.get(),
            Self::Enabled(event) => event.seq.get(),
            Self::Stopped(event) => event.seq.get(),
            Self::TaskCreated(event) => event.seq.get(),
            Self::TaskStateChanged(event) => event.seq.get(),
        }
    }

    fn queue_id(&self) -> &CommonIdV1 {
        match self {
            Self::Created(event) => &event.data.queue_id,
            Self::Enabled(event) => &event.data.queue_id,
            Self::Stopped(event) => &event.data.queue_id,
            Self::TaskCreated(event) => &event.data.queue_id,
            Self::TaskStateChanged(event) => &event.data.queue_id,
        }
    }
}

/// Rebuilds the state and the retry budget of one task from the whole stream of its queue,
/// ordered by `seq` and starting at `queue.created`. `owner_of` reads the device registry,
/// which must be append only, so that a device keeps the owner it had when it acted.
///
/// Besides the order and the budget, the replay checks the authorization of each change:
/// enabling and owner stops come from a device of the queue owner, a task enters `running`
/// only while the queue is enabled, and `review → done` is accepted by a device of the owner.
pub fn replay_task<'a>(
    task_id: &CommonIdV1,
    log: impl IntoIterator<Item = QueueLogEvent<'a>>,
    owner_of: impl Fn(&CommonIdV1) -> Option<CommonIdV1>,
) -> Option<(CommonQueueTaskStateV1, u64)> {
    let mut log = log.into_iter();
    let Some(QueueLogEvent::Created(created)) = log.next() else {
        return None;
    };
    let queue_id = &created.data.queue_id;
    let owner = &created.data.owner_id;
    let by_owner = |actor: &CommonActorV1| {
        actor.kind == CommonActorV1Kind::Device && owner_of(&actor.id).as_ref() == Some(owner)
    };
    if !by_owner(&created.actor) {
        return None;
    }
    let mut seq = created.seq.get();
    let mut queue_state = Stopped;
    // (state, attempts, ceiling) once the task is created.
    let mut task: Option<(CommonQueueTaskStateV1, u64, u64)> = None;
    for event in log {
        if event.seq() <= seq || event.queue_id() != queue_id {
            return None;
        }
        seq = event.seq();
        match event {
            QueueLogEvent::Created(_) => return None,
            QueueLogEvent::Enabled(enabled) => {
                if enabled.data.owner_id != *owner || !by_owner(&enabled.actor) {
                    return None;
                }
                queue_state = Enabled;
            }
            QueueLogEvent::Stopped(stopped) => {
                if stopped.data.reason == EventQueueStoppedV1DataReason::Owner
                    && (stopped.data.owner_id.as_ref() != Some(owner) || !by_owner(&stopped.actor))
                {
                    return None;
                }
                queue_state = Stopped;
            }
            QueueLogEvent::TaskCreated(admitted) => {
                if admitted.data.task_id != *task_id {
                    continue;
                }
                if task.is_some() {
                    return None;
                }
                task = Some((Queued, 0, admitted.data.attempt_limit.get().into()));
            }
            QueueLogEvent::TaskStateChanged(change) => {
                if change.data.task_id != *task_id {
                    continue;
                }
                let (state, attempts, limit) = task.as_mut()?;
                if change.data.from != *state {
                    return None;
                }
                if change.data.to == Running {
                    if queue_state != Enabled {
                        return None;
                    }
                    *attempts += 1;
                }
                if (*state, change.data.to) == (Review, Done) && !by_owner(&change.actor) {
                    return None;
                }
                if *attempts > *limit
                    || change.data.attempts.get() != *attempts
                    || (*state == Failed && change.data.to == Queued && *attempts == *limit)
                {
                    return None;
                }
                *state = change.data.to;
            }
        }
    }
    task.map(|(state, attempts, _)| (state, attempts))
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

/// Previously admitted origins remain reserved after termination and replay. Task ids are
/// unique across queues, and the attempt ceiling of a new task follows the configured policy.
///
/// Besides the lifetime limit, the queue admits at most `window_admissions` tasks, and at most
/// `bug_rule_window_admissions` from the same bug rule, in any `window_seconds` ending at the
/// candidate `ts`. A prior admission with a later `ts` counts as inside the window. The window
/// limits clear as time passes; the lifetime limit clears only when the owner raises it.
pub fn can_admit_task<'a>(
    created: &EventQueueTaskCreatedV1,
    history: impl IntoIterator<Item = &'a EventQueueTaskCreatedV1>,
    policy: &QueuePolicy,
) -> bool {
    if created.data.attempt_limit.get() > policy.attempt_limit.get() {
        return false;
    }
    let Some(now) = epoch_seconds(&created.ts) else {
        return false;
    };
    let window = i64::try_from(policy.window_seconds.get()).unwrap_or(i64::MAX);
    let rule = bug_rule(&created.data.origin);
    let (mut count, mut in_window, mut same_rule) = (0, 0, 0);
    for prior in history {
        if prior.data.task_id == created.data.task_id {
            return false;
        }
        if prior.data.queue_id != created.data.queue_id {
            continue;
        }
        if prior.data.workspace_alias == created.data.workspace_alias
            && prior.data.origin == created.data.origin
        {
            return false;
        }
        count += 1;
        let recent = epoch_seconds(&prior.ts).is_none_or(|then| now.saturating_sub(then) < window);
        if recent {
            in_window += 1;
            if rule.is_some() && bug_rule(&prior.data.origin) == rule {
                same_rule += 1;
            }
        }
        if count >= policy.admission_limit.get()
            || in_window >= policy.window_admissions.get()
            || same_rule >= policy.bug_rule_window_admissions.get()
        {
            return false;
        }
    }
    true
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
