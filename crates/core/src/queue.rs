//! Authorization and lifecycle policy for the task queue.

use ads_contracts::{
    CommonIdV1, CommonQueueTaskStateV1, EventQueueStoppedV1DataVia, EventQueueTaskCreatedV1,
    EventQueueTaskStateChangedV1, RecordQueueTaskV1, RecordQueueTaskV1Attempts, RecordTaskQueueV1,
    RecordTaskQueueV1State, RecordTaskQueueV1Version,
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

/// Identity and channel established by the authentication boundary.
pub struct AuthenticatedDevice {
    id: CommonIdV1,
    via: EventQueueStoppedV1DataVia,
}

impl AuthenticatedDevice {
    pub fn id(&self) -> &CommonIdV1 {
        &self.id
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
        owner_id: owner.id.clone(),
        state: Stopped,
    }
}

pub fn restore_stopped(mut persisted: RecordTaskQueueV1) -> RecordTaskQueueV1 {
    persisted.state = Stopped;
    persisted
}

/// The returned state and device identify the change to append before scheduling.
pub fn change_queue_state<'a>(
    queue: &RecordTaskQueueV1,
    device: Option<&'a AuthenticatedDevice>,
    next: RecordTaskQueueV1State,
) -> Result<(RecordTaskQueueV1, &'a AuthenticatedDevice), QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    if device.id != queue.owner_id {
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

/// Previously admitted origins remain reserved after termination and replay.
pub fn can_admit_task<'a>(
    created: &EventQueueTaskCreatedV1,
    history: impl IntoIterator<Item = &'a EventQueueTaskCreatedV1>,
    queue_limit: usize,
) -> bool {
    let mut count = 0;
    for prior in history {
        if prior.data.queue_id != created.data.queue_id {
            continue;
        }
        if prior.data.task_id == created.data.task_id
            || (prior.data.workspace_alias == created.data.workspace_alias
                && prior.data.origin == created.data.origin)
        {
            return false;
        }
        count += 1;
        if count >= queue_limit {
            return false;
        }
    }
    count < queue_limit
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
        (Review, Done) if device.is_some_and(|device| device.id == queue.owner_id) => {
            Some(task.attempts)
        }
        (Running, Review | Failed) | (Review, Failed) => Some(task.attempts),
        _ => None,
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
