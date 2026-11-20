//! Authorization and lifecycle policy for the task queue.

use ads_contracts::{
    CommonIdV1, CommonQueueTaskStateV1, EventQueueStoppedV1Data, EventQueueStoppedV1DataReason,
    EventQueueStoppedV1DataVia, EventQueueTaskCreatedV1, EventQueueTaskStateChangedV1,
    RecordQueueTaskV1, RecordQueueTaskV1Attempts, RecordTaskQueueV1, RecordTaskQueueV1State,
    RecordTaskQueueV1Version,
};

use CommonQueueTaskStateV1::{Cancelled, Done, Failed, Queued, Review, Running};
use RecordTaskQueueV1State::{Enabled, Stopped};

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/queue.toml");

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuePolicy {
    pub attempt_limit: ads_contracts::RecordQueueTaskV1AttemptLimit,
    pub admission_limit: std::num::NonZeroUsize,
}

/// Device, the owner it is registered to and channel, established by the authentication boundary.
pub struct AuthenticatedDevice {
    device_id: CommonIdV1,
    owner_id: CommonIdV1,
    via: EventQueueStoppedV1DataVia,
}

impl AuthenticatedDevice {
    pub fn device_id(&self) -> &CommonIdV1 {
        &self.device_id
    }

    pub fn owner_id(&self) -> &CommonIdV1 {
        &self.owner_id
    }

    pub fn via(&self) -> EventQueueStoppedV1DataVia {
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
    let mut stopped = queue.clone();
    stopped.state = Stopped;
    if queue.state == Stopped {
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

/// Every start restores the queue stopped and records why when the persisted state was enabled.
pub fn restore_stopped(
    persisted: RecordTaskQueueV1,
) -> (RecordTaskQueueV1, Option<EventQueueStoppedV1Data>) {
    stop_automatically(&persisted, AutomaticStop::Restart)
}

/// Data of the `queue.stopped` event for an accepted owner stop.
pub fn owner_stop_data(
    queue: &RecordTaskQueueV1,
    device: &AuthenticatedDevice,
) -> EventQueueStoppedV1Data {
    EventQueueStoppedV1Data {
        queue_id: queue.id.clone(),
        reason: EventQueueStoppedV1DataReason::Owner,
        owner_id: Some(queue.owner_id.clone()),
        via: Some(device.via),
    }
}

/// The returned state and device identify the change to append before scheduling.
pub fn change_queue_state<'a>(
    queue: &RecordTaskQueueV1,
    device: Option<&'a AuthenticatedDevice>,
    next: RecordTaskQueueV1State,
) -> Result<(RecordTaskQueueV1, &'a AuthenticatedDevice), QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    if device.owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
    }
    if next == Enabled && device.via == EventQueueStoppedV1DataVia::RemoteDevice {
        return Err(QueueRefusal::RemoteEnable);
    }
    if queue.state == next {
        return Err(QueueRefusal::Unchanged);
    }
    let mut updated = queue.clone();
    updated.state = next;
    Ok((updated, device))
}

/// Rebuilds the retry budget from an ordered, authenticated log for this task.
pub fn replay_attempts<'a>(
    created: &EventQueueTaskCreatedV1,
    changes: impl IntoIterator<Item = &'a EventQueueTaskStateChangedV1>,
) -> Option<(CommonQueueTaskStateV1, u64)> {
    let limit = u64::from(created.data.attempt_limit.get());
    let mut state = Queued;
    let mut attempts = 0;
    let mut seq = created.seq;
    for change in changes {
        if change.data.queue_id != created.data.queue_id
            || change.data.task_id != created.data.task_id
            || change.seq <= seq
            || change.data.from != state
        {
            return None;
        }
        if change.data.to == Running {
            attempts += 1;
        }
        if attempts > limit
            || change.data.attempts.get() != attempts
            || (state == Failed && change.data.to == Queued && attempts == limit)
        {
            return None;
        }
        state = change.data.to;
        seq = change.seq;
    }
    Some((state, attempts))
}

/// Previously admitted origins remain reserved after termination and replay. Task ids are
/// unique across queues, and the attempt ceiling of a new task follows the configured policy.
pub fn can_admit_task<'a>(
    created: &EventQueueTaskCreatedV1,
    history: impl IntoIterator<Item = &'a EventQueueTaskCreatedV1>,
    policy: &QueuePolicy,
) -> bool {
    if created.data.attempt_limit.get() > policy.attempt_limit.get() {
        return false;
    }
    let mut count = 0;
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
        if count >= policy.admission_limit.get() {
            return false;
        }
    }
    count < policy.admission_limit.get()
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
