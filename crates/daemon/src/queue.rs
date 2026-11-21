//! Owner control of the task queue for the local API: the state in memory and the `queue/<id>`
//! stream of the anchored audit log.

use std::sync::{Mutex, MutexGuard};

use ads_contracts::{CommonIdV1, EventEnvelopeV1, RecordTaskQueueV1, RecordTaskQueueV1State};
use ads_core::queue::{
    AuthenticatedDevice, QueueRefusal, QueueStateEvent, QueueStreamEvent, change_queue_state,
    new_stopped, restore_stopped,
};
use ads_storage::{EventLog, StorageError, queue::queue_stream};
use serde_json::{Value, json};

use crate::api::{ApiError, checked};
use crate::events::{new_id, now_ts};

use RecordTaskQueueV1State::{Enabled, Stopped};

/// What the local API asks of the queue. Every call comes from a connection that already passed
/// the token and origin checks; the owner and the device come from the implementation, never
/// from the request.
pub trait QueueControl: Send + Sync + 'static {
    fn state(&self, queue_id: &CommonIdV1) -> Result<RecordTaskQueueV1State, ApiError>;
    /// Records `queue.enabled` and only then admits tasks. `cause` is the command id.
    fn start(&self, queue_id: &CommonIdV1, cause: &str) -> Result<(), ApiError>;
    /// Always allowed: the queue stops before the event is written, and an already stopped queue
    /// gets no event.
    fn stop(&self, queue_id: &CommonIdV1, cause: &str) -> Result<(), ApiError>;
}

/// No queue bound to the audit log: every queue command is refused.
pub struct DisabledQueue;
impl QueueControl for DisabledQueue {
    fn state(&self, _: &CommonIdV1) -> Result<RecordTaskQueueV1State, ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
    fn start(&self, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
    fn stop(&self, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
}

/// Why a queue stream could not be bound.
#[derive(Debug)]
pub enum QueueOpenError {
    Storage(StorageError),
    /// The stream was created by another owner.
    ForeignOwner,
    /// The stream does not start with `queue.created` or holds a type this core does not read.
    Unreadable,
    /// An event could not be built against its contract.
    Contract,
}
impl From<StorageError> for QueueOpenError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// One queue of the local owner, persisted in the anchored log.
pub struct LoggedQueue {
    inner: Mutex<Inner>,
}
struct Inner {
    log: EventLog,
    record: RecordTaskQueueV1,
    device: AuthenticatedDevice,
    /// Last state written to the stream; differs from `record.state` only after a stop whose
    /// event could not be written.
    logged: RecordTaskQueueV1State,
}

impl LoggedQueue {
    /// Binds the queue, creating it stopped when its stream is empty. Every open restores the
    /// queue stopped and records the restart when the log last said enabled.
    pub fn open(
        log: EventLog,
        queue_id: CommonIdV1,
        device: AuthenticatedDevice,
    ) -> Result<Self, QueueOpenError> {
        let stored = log.read_queue(&queue_id)?;
        let mut inner = Inner {
            record: new_stopped(queue_id, &device),
            log,
            device,
            logged: Stopped,
        };
        if stored.is_empty() {
            let data = json!({
                "queue_id": inner.record.id, "owner_id": inner.record.owner_id,
                "via": inner.device.via().created_via(), "state": "stopped"
            });
            inner.append("created", true, data, None)?;
            return Ok(Self::wrap(inner));
        }
        let mut last = None;
        for (index, stored) in stored.iter().enumerate() {
            match QueueStreamEvent::from_envelope(&stored.event) {
                Ok(QueueStreamEvent::Created(created)) if index == 0 => {
                    if created.data.owner_id != inner.record.owner_id {
                        return Err(QueueOpenError::ForeignOwner);
                    }
                }
                Ok(QueueStreamEvent::Created(_)) | Err(_) => {
                    return Err(QueueOpenError::Unreadable);
                }
                _ if index == 0 => return Err(QueueOpenError::Unreadable),
                Ok(QueueStreamEvent::Enabled(_)) => last = Some(Enabled),
                Ok(QueueStreamEvent::Stopped(_)) => last = Some(Stopped),
                Ok(_) => {}
            }
        }
        let (record, stop) = restore_stopped(inner.record.clone(), last);
        inner.record = record;
        if let Some(data) = stop {
            let data = serde_json::to_value(data).map_err(|_| QueueOpenError::Contract)?;
            inner.append("stopped", false, data, None)?;
        }
        Ok(Self::wrap(inner))
    }

    fn wrap(inner: Inner) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }

    fn lock(&self, queue_id: &CommonIdV1) -> Result<MutexGuard<'_, Inner>, ApiError> {
        // A panic while holding the lock leaves the state readable; stopping must still work.
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.record.id != *queue_id {
            return Err(ApiError::QueueNotConfigured);
        }
        Ok(inner)
    }
}

impl Inner {
    /// Appends at the next `seq` of the queue stream; the storage refuses a stale one.
    fn append(
        &mut self,
        kind: &str,
        by_device: bool,
        data: Value,
        cause: Option<&str>,
    ) -> Result<(), QueueOpenError> {
        let stream = queue_stream(&self.record.id);
        let seq = self.log.head(&stream)? + 1;
        let actor = if by_device {
            json!({"kind": "device", "id": self.device.device_id()})
        } else {
            json!({"kind": "daemon", "id": "core"})
        };
        let schema = format!("urn:ads:event:queue.{kind}:1");
        let value = json!({
            "id": new_id().map_err(|_| QueueOpenError::Contract)?, "type": schema,
            "ts": now_ts().map_err(|_| QueueOpenError::Contract)?, "stream": stream.as_str(),
            "seq": seq, "actor": actor, "cause": cause, "data_class": "internal", "data": data
        });
        let value = checked(&schema, value).map_err(|_| QueueOpenError::Contract)?;
        let event: EventEnvelopeV1 =
            serde_json::from_value(value).map_err(|_| QueueOpenError::Contract)?;
        self.log.append(&event)?;
        Ok(())
    }

    fn append_change(&mut self, event: QueueStateEvent, cause: &str) -> Result<(), ApiError> {
        let (kind, data) = match event {
            QueueStateEvent::Enabled(data) => ("enabled", serde_json::to_value(data)),
            QueueStateEvent::Stopped(data) => ("stopped", serde_json::to_value(data)),
        };
        let data = data.map_err(|_| ApiError::Storage)?;
        self.append(kind, true, data, Some(cause))
            .map_err(|_| ApiError::Storage)
    }

    /// Writes the owner stop that an earlier failure left out of the stream.
    fn settle_stop(&mut self, cause: &str) -> Result<(), ApiError> {
        if self.logged == Stopped {
            return Ok(());
        }
        let mut enabled = self.record.clone();
        enabled.state = Enabled;
        let (_, event, _) = change_queue_state(&enabled, Some(&self.device), Stopped)
            .map_err(|_| ApiError::QueueNotConfigured)?;
        self.append_change(event, cause)?;
        self.logged = Stopped;
        Ok(())
    }
}

impl QueueControl for LoggedQueue {
    fn state(&self, queue_id: &CommonIdV1) -> Result<RecordTaskQueueV1State, ApiError> {
        Ok(self.lock(queue_id)?.record.state)
    }

    fn start(&self, queue_id: &CommonIdV1, cause: &str) -> Result<(), ApiError> {
        let mut inner = self.lock(queue_id)?;
        inner.settle_stop(cause)?;
        let (updated, event, _) = change_queue_state(&inner.record, Some(&inner.device), Enabled)
            .map_err(|refusal| match refusal {
            QueueRefusal::Unchanged => ApiError::QueueAlreadyEnabled,
            _ => ApiError::QueueNotConfigured,
        })?;
        inner.append_change(event, cause)?;
        inner.record = updated;
        inner.logged = Enabled;
        Ok(())
    }

    fn stop(&self, queue_id: &CommonIdV1, cause: &str) -> Result<(), ApiError> {
        let mut inner = self.lock(queue_id)?;
        if inner.record.state == Stopped {
            return inner.settle_stop(cause);
        }
        let (updated, event, _) = change_queue_state(&inner.record, Some(&inner.device), Stopped)
            .map_err(|_| ApiError::QueueNotConfigured)?;
        // Stopped before the write: a failed append never leaves the queue admitting tasks.
        inner.record = updated;
        inner.append_change(event, cause)?;
        inner.logged = Stopped;
        Ok(())
    }
}
