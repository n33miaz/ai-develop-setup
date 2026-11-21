//! Owner control of the task queue for the local API: the state in memory and the `queue/<id>`
//! stream of the anchored audit log.

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, OnceLock};

use ads_contracts::{
    CommonIdV1, EventEnvelopeV1, RecordTaskQueueV1, RecordTaskQueueV1State,
    RecordTaskQueueV1Version,
};
use ads_core::queue::{
    DEFAULT_POLICY, QueuePolicy, QueueRefusal, QueueStateEvent, QueueStreamEvent,
    change_local_queue_state, new_local_stopped, restore_stopped,
};
use ads_storage::{EventLog, StorageError, queue::queue_stream};
use serde_json::{Value, json};

use crate::api::{ApiError, checked};
use crate::auth::LocalSession;
use crate::events::{new_id, now_ts};

use RecordTaskQueueV1State::{Enabled, Stopped};

/// What the local API asks of the queue. Each call takes the proof that the connection passed
/// the token check; the owner and the device come from the implementation, never the request.
pub trait QueueControl: Send + Sync + 'static {
    fn state(
        &self,
        session: &LocalSession,
        queue_id: &CommonIdV1,
    ) -> Result<RecordTaskQueueV1State, ApiError>;
    /// Records `queue.enabled` and only then admits tasks. `cause` is the command id.
    fn start(
        &self,
        session: &LocalSession,
        queue_id: &CommonIdV1,
        cause: &str,
    ) -> Result<(), ApiError>;
    /// Always leaves the queue stopped, stopping it before the event is written. Returns whether
    /// this call stopped it; an already stopped queue gets no event.
    fn stop(
        &self,
        session: &LocalSession,
        queue_id: &CommonIdV1,
        cause: &str,
    ) -> Result<bool, ApiError>;
}

/// No queue bound to the audit log: every queue command is refused.
pub struct DisabledQueue;
impl QueueControl for DisabledQueue {
    fn state(&self, _: &LocalSession, _: &CommonIdV1) -> Result<RecordTaskQueueV1State, ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
    fn start(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
    fn stop(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<bool, ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
}

/// The owner of the queue and the device id the local interface acts as, from the core's own
/// configuration. Plain data: creating and acting on the queue still take a [`LocalSession`].
#[derive(Debug, Clone)]
pub struct QueueOwner {
    pub owner_id: CommonIdV1,
    pub device_id: CommonIdV1,
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
    /// This process already holds the queue; share that handle instead.
    AlreadyOpen,
    /// The stream is empty and the caller gave no session proof to create the queue.
    NotCreated,
}
impl From<StorageError> for QueueOpenError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// Queues held by this process. Two handles would keep two states for one stream.
fn held() -> MutexGuard<'static, HashSet<String>> {
    static HELD: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    HELD.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// One queue of the local owner, persisted in the anchored log. At most one per queue id lives
/// in a process at a time.
pub struct LoggedQueue {
    inner: Mutex<Inner>,
}
struct Inner {
    log: EventLog,
    record: RecordTaskQueueV1,
    owner: QueueOwner,
    /// Id of the stop command whose event could not be written; the event is written later
    /// with this cause, before any other change by this process.
    pending_stop: Option<String>,
}

impl Drop for LoggedQueue {
    fn drop(&mut self) {
        let inner = self.inner.get_mut().unwrap_or_else(|e| e.into_inner());
        held().remove(inner.record.id.as_str());
    }
}

impl LoggedQueue {
    /// Binds a queue whose stream already exists, restoring it stopped and recording the restart
    /// when the log last said enabled. An empty stream is refused with `NotCreated` and nothing is
    /// written: creating the queue is an owner act and takes a [`LocalSession`].
    pub fn bind(
        log: EventLog,
        queue_id: CommonIdV1,
        owner: QueueOwner,
    ) -> Result<Self, QueueOpenError> {
        Self::acquire(log, queue_id, owner, None)
    }

    /// Like [`Self::open_with_policy`], with the bundled policy for a new queue.
    ///
    /// Creating a queue as the owner's device takes the session proof, so this does not compile:
    ///
    /// ```compile_fail,E0061
    /// use ads_contracts::CommonIdV1;
    /// use ads_daemon::queue::{LoggedQueue, QueueOwner};
    /// fn create(log: ads_storage::EventLog, id: CommonIdV1, owner: QueueOwner) {
    ///     let _ = LoggedQueue::open(log, id, owner);
    /// }
    /// ```
    ///
    /// ```
    /// use ads_contracts::CommonIdV1;
    /// use ads_daemon::{auth::LocalSession, queue::{LoggedQueue, QueueOwner}};
    /// fn create(s: &LocalSession, log: ads_storage::EventLog, id: CommonIdV1, owner: QueueOwner) {
    ///     let _ = LoggedQueue::open(s, log, id, owner);
    /// }
    /// ```
    pub fn open(
        session: &LocalSession,
        log: EventLog,
        queue_id: CommonIdV1,
        owner: QueueOwner,
    ) -> Result<Self, QueueOpenError> {
        let policy = toml::from_str(DEFAULT_POLICY).expect("bundled queue policy is valid");
        Self::open_with_policy(session, log, queue_id, owner, &policy)
    }

    /// Binds the queue, creating it stopped with `initial_policy` when its stream is empty. The
    /// creation is recorded as an act of the owner's device, so it takes the same session proof
    /// as starting and stopping the queue.
    pub fn open_with_policy(
        _: &LocalSession,
        log: EventLog,
        queue_id: CommonIdV1,
        owner: QueueOwner,
        initial_policy: &QueuePolicy,
    ) -> Result<Self, QueueOpenError> {
        Self::acquire(log, queue_id, owner, Some(initial_policy))
    }

    fn acquire(
        log: EventLog,
        queue_id: CommonIdV1,
        owner: QueueOwner,
        create: Option<&QueuePolicy>,
    ) -> Result<Self, QueueOpenError> {
        if !held().insert(queue_id.as_str().to_owned()) {
            return Err(QueueOpenError::AlreadyOpen);
        }
        let release = |error| {
            held().remove(queue_id.as_str());
            error
        };
        let inner = Inner {
            record: RecordTaskQueueV1 {
                version: RecordTaskQueueV1Version,
                id: queue_id.clone(),
                owner_id: owner.owner_id.clone(),
                state: Stopped,
            },
            log,
            owner,
            pending_stop: None,
        };
        let inner = Self::restore(inner, create).map_err(release)?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    fn restore(mut inner: Inner, create: Option<&QueuePolicy>) -> Result<Inner, QueueOpenError> {
        let stored = inner.log.read_queue(&inner.record.id)?;
        if stored.is_empty() {
            let initial_policy = create.ok_or(QueueOpenError::NotCreated)?;
            let (record, created) = new_local_stopped(
                inner.record.id.clone(),
                &inner.owner.owner_id,
                initial_policy,
            );
            let data = serde_json::to_value(created).map_err(|_| QueueOpenError::Contract)?;
            inner.append_at(1, "created:2", true, data, None)?;
            inner.record = record;
            return Ok(inner);
        }
        let mut last = None;
        for (index, stored) in stored.iter().enumerate() {
            match QueueStreamEvent::from_envelope(&stored.event) {
                Ok(QueueStreamEvent::Created(created)) if index == 0 => {
                    if created.data.owner_id != inner.record.owner_id {
                        return Err(QueueOpenError::ForeignOwner);
                    }
                }
                Ok(QueueStreamEvent::CreatedV2(created)) if index == 0 => {
                    if created.data.owner_id != inner.record.owner_id {
                        return Err(QueueOpenError::ForeignOwner);
                    }
                }
                Ok(QueueStreamEvent::Created(_) | QueueStreamEvent::CreatedV2(_)) | Err(_) => {
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
            inner.append("stopped:1", false, data, None)?;
        }
        Ok(inner)
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
        self.append_at(seq, kind, by_device, data, cause)
    }

    fn append_at(
        &mut self,
        seq: u64,
        kind: &str,
        by_device: bool,
        data: Value,
        cause: Option<&str>,
    ) -> Result<(), QueueOpenError> {
        let stream = queue_stream(&self.record.id);
        let actor = if by_device {
            json!({"kind": "device", "id": self.owner.device_id})
        } else {
            json!({"kind": "daemon", "id": "core"})
        };
        let schema = format!("urn:ads:event:queue.{kind}");
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

    fn change(
        &self,
        next: RecordTaskQueueV1State,
    ) -> Result<(RecordTaskQueueV1, QueueStateEvent), QueueRefusal> {
        change_local_queue_state(&self.record, &self.owner.owner_id, next)
    }

    fn append_change(&mut self, event: QueueStateEvent, cause: &str) -> Result<(), ApiError> {
        let (kind, data) = match event {
            QueueStateEvent::Enabled(data) => ("enabled:1", serde_json::to_value(data)),
            QueueStateEvent::Stopped(data) => ("stopped:1", serde_json::to_value(data)),
        };
        let data = data.map_err(|_| ApiError::Storage)?;
        self.append(kind, true, data, Some(cause))
            .map_err(|_| ApiError::Storage)
    }

    /// Writes the owner stop that an earlier failure left out of the stream.
    fn settle_stop(&mut self) -> Result<(), ApiError> {
        let Some(cause) = self.pending_stop.clone() else {
            return Ok(());
        };
        let mut enabled = self.record.clone();
        enabled.state = Enabled;
        let (_, event) = change_local_queue_state(&enabled, &self.owner.owner_id, Stopped)
            .map_err(|_| ApiError::QueueNotConfigured)?;
        self.append_change(event, &cause)?;
        self.pending_stop = None;
        Ok(())
    }
}

impl QueueControl for LoggedQueue {
    fn state(
        &self,
        _: &LocalSession,
        queue_id: &CommonIdV1,
    ) -> Result<RecordTaskQueueV1State, ApiError> {
        Ok(self.lock(queue_id)?.record.state)
    }

    fn start(&self, _: &LocalSession, queue_id: &CommonIdV1, cause: &str) -> Result<(), ApiError> {
        let mut inner = self.lock(queue_id)?;
        inner.settle_stop()?;
        let (updated, event) = inner.change(Enabled).map_err(|refusal| match refusal {
            QueueRefusal::Unchanged => ApiError::QueueAlreadyEnabled,
            _ => ApiError::QueueNotConfigured,
        })?;
        inner.append_change(event, cause)?;
        inner.record = updated;
        Ok(())
    }

    fn stop(&self, _: &LocalSession, queue_id: &CommonIdV1, cause: &str) -> Result<bool, ApiError> {
        let mut inner = self.lock(queue_id)?;
        if inner.record.state == Stopped {
            inner.settle_stop()?;
            return Ok(false);
        }
        let (updated, _) = inner
            .change(Stopped)
            .map_err(|_| ApiError::QueueNotConfigured)?;
        // Stopped before the write: a failed append never leaves the queue admitting tasks.
        inner.record = updated;
        inner.pending_stop = Some(cause.to_owned());
        inner.settle_stop()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Token;
    use ads_storage::StorageConfig;
    use ads_storage::anchor::{AnchorStore, Commitment, DatabaseId};
    use ads_storage::cipher::{CipherError, KeyId, KeyProvider, SecretKey};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct Keys;
    impl KeyProvider for Keys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new("key-a")?, SecretKey::new(&[1; 32])))
        }
        fn by_id(&self, _: &KeyId) -> Result<SecretKey, CipherError> {
            Ok(SecretKey::new(&[1; 32]))
        }
    }

    #[derive(Clone, Default)]
    struct Anchor(Arc<Mutex<Option<Commitment>>>);
    impl AnchorStore for Anchor {
        fn can_create(&self) -> Result<bool, StorageError> {
            Ok(self.0.lock().unwrap().is_none())
        }
        fn read(&self, _: DatabaseId) -> Result<Option<Commitment>, StorageError> {
            Ok(*self.0.lock().unwrap())
        }
        fn store(&self, _: DatabaseId, commitment: &Commitment) -> Result<(), StorageError> {
            *self.0.lock().unwrap() = Some(*commitment);
            Ok(())
        }
    }

    /// A database inside the repository's `.test-tmp`, removed file by file on drop.
    struct Folder(PathBuf);
    impl Folder {
        fn new(name: &str) -> Self {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
            std::fs::create_dir_all(&root).unwrap();
            let root = root.canonicalize().unwrap();
            assert!(root.ends_with(".test-tmp"));
            let dir = root.join(format!("{name}-{}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn log(&self, anchor: &Anchor) -> EventLog {
            EventLog::open(
                &self.0.join("audit.db"),
                &StorageConfig::defaults(),
                Box::new(Keys),
                Box::new(anchor.clone()),
            )
            .unwrap()
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            for name in [
                "audit.db",
                "audit.db-wal",
                "audit.db-shm",
                "audit.db-journal",
            ] {
                let _ = std::fs::remove_file(self.0.join(name));
            }
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    fn id(text: &str) -> CommonIdV1 {
        CommonIdV1::new(text).unwrap()
    }

    fn owner() -> QueueOwner {
        QueueOwner {
            owner_id: id("owner-01"),
            device_id: id("device-01"),
        }
    }

    #[test]
    fn only_a_session_creates_the_queue_and_records_its_policy() {
        let folder = Folder::new("queue-unit-create");
        let anchor = Anchor::default();
        let queue = id("queue-unit-create");
        assert!(matches!(
            LoggedQueue::bind(folder.log(&anchor), queue.clone(), owner()),
            Err(QueueOpenError::NotCreated)
        ));
        assert!(folder.log(&anchor).read_queue(&queue).unwrap().is_empty());

        let token = Token::generate().unwrap();
        let session = token.open_session("forged");
        assert!(session.is_none());
        let session = token.open_session(&token_text(&token)).unwrap();
        let policy: QueuePolicy =
            toml::from_str("attempt_limit = 2\nadmission_limit = 1000\n").unwrap();
        let created = LoggedQueue::open_with_policy(
            &session,
            folder.log(&anchor),
            queue.clone(),
            owner(),
            &policy,
        )
        .unwrap();
        drop(created);

        let stored = folder.log(&anchor).read_queue(&queue).unwrap();
        let event = serde_json::to_value(&stored[0].event).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(event["type"], "urn:ads:event:queue.created:2");
        assert_eq!(event["actor"], json!({"kind": "device", "id": "device-01"}));
        assert_eq!(event["data"]["policy"]["attempt_limit"], 2);
        drop(LoggedQueue::bind(folder.log(&anchor), queue.clone(), owner()).unwrap());
        // An existing stream keeps the policy of its creation.
        let reopened = LoggedQueue::open(&session, folder.log(&anchor), queue.clone(), owner());
        drop(reopened.unwrap());
        assert_eq!(folder.log(&anchor).read_queue(&queue).unwrap().len(), 1);
    }

    /// The run token as the UI receives it, read back from the launch URL.
    fn token_text(token: &Token) -> String {
        let url = token.launch_url("http://127.0.0.1:5173", 1);
        url.rsplit("token=").next().unwrap().to_owned()
    }
}
