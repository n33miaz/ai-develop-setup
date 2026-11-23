//! Owner control of the task queue for the local API: the state in memory and the `queue/<id>`
//! stream of the anchored audit log.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use ads_contracts::{
    CommandQueueGetResponseV2Result, CommonIdV1, CommonQueuePolicyV1, EventEnvelopeV1,
    RecordTaskQueueV1, RecordTaskQueueV1State, RecordTaskQueueV1Version,
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
    /// The id of the queue this core serves, created or not.
    fn configured(&self, _: &LocalSession) -> Result<CommonIdV1, ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
    fn create(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        Err(ApiError::QueueNotConfigured)
    }
    fn state(
        &self,
        session: &LocalSession,
        queue_id: &CommonIdV1,
    ) -> Result<RecordTaskQueueV1State, ApiError>;
    fn get(
        &self,
        session: &LocalSession,
        queue_id: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError>;
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
    fn get(
        &self,
        _: &LocalSession,
        _: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError> {
        Err(ApiError::QueueNotConfigured)
    }

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
    AlreadyCreated,
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
    log: Arc<Mutex<EventLog>>,
    busy_timeout: std::time::Duration,
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

enum Opening<'a> {
    Bind(std::time::Duration),
    Open(&'a QueuePolicy),
    Create {
        policy: &'a QueuePolicy,
        cause: &'a str,
        timeout: std::time::Duration,
    },
}

impl LoggedQueue {
    /// Binds a queue whose stream already exists, restoring it stopped and recording the restart
    /// when the log last said enabled. An empty stream is refused with `NotCreated` and nothing is
    /// written: creating the queue is an owner act and takes a [`LocalSession`].
    /// Supply the log's configured busy timeout to bound the initial lock wait.
    pub fn bind(
        log: Arc<Mutex<EventLog>>,
        queue_id: CommonIdV1,
        owner: QueueOwner,
        busy_timeout: std::time::Duration,
    ) -> Result<Self, QueueOpenError> {
        Self::acquire(log, queue_id, owner, Opening::Bind(busy_timeout))
    }

    /// Like [`Self::open_with_policy`], with the bundled policy for a new queue.
    ///
    /// Creating a queue as the owner's device takes the session proof, so this does not compile:
    ///
    /// ```compile_fail
    /// use ads_contracts::CommonIdV1;
    /// use ads_daemon::queue::{LoggedQueue, QueueOwner};
    /// fn create(log: std::sync::Arc<std::sync::Mutex<ads_storage::EventLog>>, id: CommonIdV1, owner: QueueOwner) {
    ///     let _ = LoggedQueue::open(log, id, owner);
    /// }
    /// ```
    ///
    /// The same call with the proof compiles:
    ///
    /// ```
    /// use ads_contracts::CommonIdV1;
    /// use ads_daemon::{auth::LocalSession, queue::{LoggedQueue, QueueOwner}};
    /// fn create(s: &LocalSession, log: std::sync::Arc<std::sync::Mutex<ads_storage::EventLog>>, id: CommonIdV1, owner: QueueOwner) {
    ///     let _ = LoggedQueue::open(s, log, id, owner);
    /// }
    /// ```
    pub fn open(
        session: &LocalSession,
        log: Arc<Mutex<EventLog>>,
        queue_id: CommonIdV1,
        owner: QueueOwner,
    ) -> Result<Self, QueueOpenError> {
        let policy = toml::from_str(DEFAULT_POLICY).expect("bundled queue policy is valid");
        Self::open_with_policy(session, log, queue_id, owner, &policy)
    }

    /// Binds the queue, creating it stopped with `initial_policy` when its stream is empty. The
    /// creation is recorded as an act of the owner's device, so it takes the same session proof
    /// as starting and stopping the queue.
    ///
    /// ```compile_fail
    /// use ads_contracts::CommonIdV1;
    /// use ads_core::queue::QueuePolicy;
    /// use ads_daemon::queue::{LoggedQueue, QueueOwner};
    /// fn create(log: std::sync::Arc<std::sync::Mutex<ads_storage::EventLog>>, id: CommonIdV1, owner: QueueOwner, p: &QueuePolicy) {
    ///     let _ = LoggedQueue::open_with_policy(log, id, owner, p);
    /// }
    /// ```
    ///
    /// ```
    /// use ads_contracts::CommonIdV1;
    /// use ads_core::queue::QueuePolicy;
    /// use ads_daemon::{auth::LocalSession, queue::{LoggedQueue, QueueOwner}};
    /// fn create(
    ///     s: &LocalSession,
    ///     log: std::sync::Arc<std::sync::Mutex<ads_storage::EventLog>>,
    ///     id: CommonIdV1,
    ///     owner: QueueOwner,
    ///     p: &QueuePolicy,
    /// ) {
    ///     let _ = LoggedQueue::open_with_policy(s, log, id, owner, p);
    /// }
    /// ```
    pub fn open_with_policy(
        _: &LocalSession,
        log: Arc<Mutex<EventLog>>,
        queue_id: CommonIdV1,
        owner: QueueOwner,
        initial_policy: &QueuePolicy,
    ) -> Result<Self, QueueOpenError> {
        Self::acquire(log, queue_id, owner, Opening::Open(initial_policy))
    }

    fn acquire(
        log: Arc<Mutex<EventLog>>,
        queue_id: CommonIdV1,
        owner: QueueOwner,
        opening: Opening<'_>,
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
            busy_timeout: std::time::Duration::ZERO,
            owner,
            pending_stop: None,
        };
        let inner = Self::restore(inner, opening).map_err(release)?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    fn restore(mut inner: Inner, opening: Opening<'_>) -> Result<Inner, QueueOpenError> {
        let stored = {
            let log = match opening {
                Opening::Bind(timeout) | Opening::Create { timeout, .. } => {
                    crate::audit::lock_with_timeout(&inner.log, timeout)?
                }
                Opening::Open(_) => inner.log.lock().map_err(|_| StorageError::Unavailable)?,
            };
            inner.busy_timeout = std::time::Duration::from_millis(log.busy_timeout_ms().into());
            log.read_queue(&inner.record.id)?
        };
        if stored.is_empty() {
            let (initial_policy, cause) = match opening {
                Opening::Bind(_) => return Err(QueueOpenError::NotCreated),
                Opening::Open(policy) => (policy, None),
                Opening::Create { policy, cause, .. } => (policy, Some(cause)),
            };
            let (record, created) = new_local_stopped(
                inner.record.id.clone(),
                &inner.owner.owner_id,
                initial_policy,
            );
            let data = serde_json::to_value(created).map_err(|_| QueueOpenError::Contract)?;
            inner.append_at(Some(1), "created:2", true, data, cause)?;
            inner.record = record;
            return Ok(inner);
        }
        if matches!(opening, Opening::Create { .. }) {
            return Err(QueueOpenError::AlreadyCreated);
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
            return Err(ApiError::QueueNotFound);
        }
        Ok(inner)
    }
}

impl Inner {
    fn recorded_policy(&self) -> Result<Option<CommonQueuePolicyV1>, ApiError> {
        let stored = crate::audit::lock_with_timeout(&self.log, self.busy_timeout)
            .and_then(|log| log.read_queue(&self.record.id))
            .map_err(ApiError::from_storage)?;
        for stored in stored.iter().rev() {
            let event =
                QueueStreamEvent::from_envelope(&stored.event).map_err(|_| ApiError::Storage)?;
            match event {
                QueueStreamEvent::PolicyChanged(changed) => {
                    if changed.data.owner_id != self.record.owner_id {
                        return Err(ApiError::Storage);
                    }
                    return Ok(Some(changed.into_fields().data.policy));
                }
                QueueStreamEvent::CreatedV2(created) => {
                    return Ok(Some(created.into_fields().data.policy));
                }
                QueueStreamEvent::Created(_) => return Ok(None),
                _ => {}
            }
        }
        Err(ApiError::Storage)
    }

    /// Appends at the next `seq` of the queue stream; the storage refuses a stale one.
    fn append(
        &mut self,
        kind: &str,
        by_device: bool,
        data: Value,
        cause: Option<&str>,
    ) -> Result<(), QueueOpenError> {
        self.append_at(None, kind, by_device, data, cause)
    }

    fn append_at(
        &mut self,
        seq: Option<u64>,
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
        let mut log = crate::audit::lock_with_timeout(&self.log, self.busy_timeout)?;
        let seq = match seq {
            Some(seq) => seq,
            None => log.head(&stream)? + 1,
        };
        let value = json!({
            "id": new_id().map_err(|_| QueueOpenError::Contract)?, "type": schema,
            "ts": now_ts().map_err(|_| QueueOpenError::Contract)?, "stream": stream.as_str(),
            "seq": seq, "actor": actor, "cause": cause, "data_class": "internal", "data": data
        });
        let value = checked(&schema, value).map_err(|_| QueueOpenError::Contract)?;
        let event: EventEnvelopeV1 =
            serde_json::from_value(value).map_err(|_| QueueOpenError::Contract)?;
        log.append(&event)?;
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
            .map_err(|error| match error {
                QueueOpenError::Storage(error) => ApiError::from_storage(error),
                _ => ApiError::Storage,
            })
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

/// One configured queue, bound on startup or awaiting an authenticated create command.
pub struct LocalQueue {
    log: Arc<Mutex<EventLog>>,
    id: CommonIdV1,
    owner: QueueOwner,
    bound: Mutex<Option<LoggedQueue>>,
    busy_timeout: std::time::Duration,
}

impl LocalQueue {
    /// Uses the log's configured busy timeout to bind the stream or detect that it is empty.
    pub fn bind_or_pending(
        log: Arc<Mutex<EventLog>>,
        id: CommonIdV1,
        owner: QueueOwner,
        busy_timeout: std::time::Duration,
    ) -> Result<Self, QueueOpenError> {
        let bound = match LoggedQueue::bind(log.clone(), id.clone(), owner.clone(), busy_timeout) {
            Ok(queue) => Some(queue),
            Err(QueueOpenError::NotCreated) => None,
            Err(error) => return Err(error),
        };
        Ok(Self {
            log,
            id,
            owner,
            bound: Mutex::new(bound),
            busy_timeout,
        })
    }

    fn lock(&self, id: &CommonIdV1) -> Result<MutexGuard<'_, Option<LoggedQueue>>, ApiError> {
        if self.id != *id {
            return Err(ApiError::QueueNotFound);
        }
        self.bound.lock().map_err(|_| ApiError::Storage)
    }
}

impl QueueControl for LocalQueue {
    fn configured(&self, _: &LocalSession) -> Result<CommonIdV1, ApiError> {
        Ok(self.id.clone())
    }

    fn create(&self, _: &LocalSession, id: &CommonIdV1, cause: &str) -> Result<(), ApiError> {
        let mut bound = self.lock(id)?;
        if bound.is_some() {
            return Err(ApiError::QueueAlreadyCreated);
        }
        let policy = toml::from_str(DEFAULT_POLICY).expect("bundled queue policy is valid");
        let queue = LoggedQueue::acquire(
            self.log.clone(),
            id.clone(),
            self.owner.clone(),
            Opening::Create {
                policy: &policy,
                cause,
                timeout: self.busy_timeout,
            },
        )
        .map_err(|error| match error {
            QueueOpenError::AlreadyCreated => ApiError::QueueAlreadyCreated,
            QueueOpenError::Storage(error) => ApiError::from_storage(error),
            _ => ApiError::Storage,
        })?;
        *bound = Some(queue);
        Ok(())
    }

    fn state(
        &self,
        session: &LocalSession,
        id: &CommonIdV1,
    ) -> Result<RecordTaskQueueV1State, ApiError> {
        self.lock(id)?
            .as_ref()
            .ok_or(ApiError::QueueNotCreated)?
            .state(session, id)
    }

    fn get(
        &self,
        session: &LocalSession,
        id: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError> {
        self.lock(id)?
            .as_ref()
            .ok_or(ApiError::QueueNotCreated)?
            .get(session, id)
    }

    fn start(&self, session: &LocalSession, id: &CommonIdV1, cause: &str) -> Result<(), ApiError> {
        self.lock(id)?
            .as_ref()
            .ok_or(ApiError::QueueNotCreated)?
            .start(session, id, cause)
    }

    fn stop(&self, session: &LocalSession, id: &CommonIdV1, cause: &str) -> Result<bool, ApiError> {
        self.lock(id)?
            .as_ref()
            .ok_or(ApiError::QueueNotCreated)?
            .stop(session, id, cause)
    }
}

impl QueueControl for LoggedQueue {
    fn configured(&self, _: &LocalSession) -> Result<CommonIdV1, ApiError> {
        Ok(self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record
            .id
            .clone())
    }

    fn create(&self, _: &LocalSession, id: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        drop(self.lock(id)?);
        Err(ApiError::QueueAlreadyCreated)
    }

    fn state(
        &self,
        _: &LocalSession,
        queue_id: &CommonIdV1,
    ) -> Result<RecordTaskQueueV1State, ApiError> {
        Ok(self.lock(queue_id)?.record.state)
    }

    fn get(
        &self,
        _: &LocalSession,
        queue_id: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError> {
        use ads_contracts::CommandQueueGetResponseV2ResultState;

        let inner = self.lock(queue_id)?;
        let state = match inner.record.state {
            Stopped => CommandQueueGetResponseV2ResultState::Stopped,
            Enabled => CommandQueueGetResponseV2ResultState::Enabled,
        };
        Ok(CommandQueueGetResponseV2Result {
            queue_id: queue_id.clone(),
            state,
            policy: inner.recorded_policy()?,
        })
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

    fn session(token: &Token) -> LocalSession {
        assert!(token.open_session("forged").is_none());
        token.open_session(&token_text(token)).unwrap()
    }

    fn policy(text: &str) -> QueuePolicy {
        toml::from_str(text).unwrap()
    }

    /// Every key differs from `defaults/queue.toml`.
    const CHOSEN: &str = "attempt_limit = 5\nadmission_limit = 77\nwindow_seconds = 1800\n\
        window_admissions = 4\nmanual_window_admissions = 6\nbug_rule_window_admissions = 2\n\
        clock_skew_seconds = 7\nclock_rewind_seconds = 120\nhost_confirm_seconds = 30\n";

    fn stored(folder: &Folder, anchor: &Anchor, queue: &CommonIdV1) -> Vec<Value> {
        folder
            .log(anchor)
            .read_queue(queue)
            .unwrap()
            .into_iter()
            .map(|stored| serde_json::to_value(stored.event).unwrap())
            .collect()
    }

    #[test]
    fn only_a_session_creates_the_queue_and_records_its_policy() {
        let folder = Folder::new("queue-unit-create");
        let anchor = Anchor::default();
        let queue = id("queue-unit-create");
        let bind = || {
            let log = folder.log(&anchor);
            let busy_timeout = std::time::Duration::from_millis(log.busy_timeout_ms().into());
            LoggedQueue::bind(
                Arc::new(Mutex::new(log)),
                queue.clone(),
                owner(),
                busy_timeout,
            )
        };
        assert!(matches!(bind(), Err(QueueOpenError::NotCreated)));
        assert!(stored(&folder, &anchor, &queue).is_empty());

        let token = Token::generate().unwrap();
        let session = session(&token);
        let open = |policy: &QueuePolicy| {
            LoggedQueue::open_with_policy(
                &session,
                Arc::new(Mutex::new(folder.log(&anchor))),
                queue.clone(),
                owner(),
                policy,
            )
            .unwrap()
        };
        drop(open(&policy(CHOSEN)));
        let events = stored(&folder, &anchor, &queue);
        let [event] = &events[..] else {
            panic!("expected only the creation");
        };
        assert_eq!(event["type"], "urn:ads:event:queue.created:2");
        assert_eq!(event["actor"], json!({"kind": "device", "id": "device-01"}));
        assert_eq!(
            event["data"]["policy"],
            json!({
                "attempt_limit": 5, "admission_limit": 77, "window_seconds": 1800,
                "window_admissions": 4, "manual_window_admissions": 6,
                "bug_rule_window_admissions": 2, "clock_skew_seconds": 7,
                "clock_rewind_seconds": 120, "host_confirm_seconds": 30
            })
        );

        // An existing stream keeps its creation, whatever policy the reopening offers.
        drop(bind().unwrap());
        drop(open(&policy("attempt_limit = 9\nadmission_limit = 5\n")));
        drop(
            LoggedQueue::open(
                &session,
                Arc::new(Mutex::new(folder.log(&anchor))),
                queue.clone(),
                owner(),
            )
            .unwrap(),
        );
        assert_eq!(stored(&folder, &anchor, &queue), events);
    }

    #[test]
    fn open_creates_a_new_queue_with_the_bundled_policy() {
        let folder = Folder::new("queue-unit-bundled");
        let anchor = Anchor::default();
        let queue = id("queue-unit-bundled");
        let token = Token::generate().unwrap();
        let opened = LoggedQueue::open(
            &session(&token),
            Arc::new(Mutex::new(folder.log(&anchor))),
            queue.clone(),
            owner(),
        );
        drop(opened.unwrap());
        let bundled: toml::Table = toml::from_str(DEFAULT_POLICY).unwrap();
        let events = stored(&folder, &anchor, &queue);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0]["data"]["policy"],
            serde_json::to_value(bundled).unwrap()
        );
    }

    #[test]
    fn a_lowered_policy_on_reopening_keeps_the_admitted_task_and_the_stored_policy() {
        let folder = Folder::new("queue-unit-lowered");
        let anchor = Anchor::default();
        let queue = id("queue-unit-lowered");
        let token = Token::generate().unwrap();
        let session = session(&token);
        let initial = policy("attempt_limit = 3\nadmission_limit = 1000\n");
        let lowered = policy("attempt_limit = 1\nadmission_limit = 1\n");
        let open = |policy: &QueuePolicy| {
            LoggedQueue::open_with_policy(
                &session,
                Arc::new(Mutex::new(folder.log(&anchor))),
                queue.clone(),
                owner(),
                policy,
            )
            .unwrap()
        };
        drop(open(&initial));
        let task = serde_json::from_value(json!({
            "id": new_id().unwrap(), "type": "urn:ads:event:queue.task_created:1",
            "ts": now_ts().unwrap(), "stream": queue_stream(&queue).as_str(), "seq": 2,
            "actor": {"kind": "device", "id": "device-01"}, "cause": null,
            "data_class": "internal",
            "data": {
                "queue_id": queue.as_str(), "task_id": "task-01", "attempt_limit": 3,
                "origin": {"kind": "manual", "request_id": "request-01"},
                "workspace_alias": "docs", "state": "queued", "attempts": 0
            }
        }))
        .unwrap();
        folder.log(&anchor).append(&task).unwrap();
        let before = stored(&folder, &anchor, &queue);

        drop(open(&lowered));
        assert_eq!(stored(&folder, &anchor, &queue), before);
        let events: Vec<_> = folder
            .log(&anchor)
            .read_queue(&queue)
            .unwrap()
            .iter()
            .map(|stored| QueueStreamEvent::from_envelope(&stored.event).unwrap())
            .collect();
        let owner_at = |device: &CommonIdV1, _: &ads_contracts::CommonTimestampV1| {
            (device.as_str() == "device-01").then(|| id("owner-01"))
        };
        let replayed =
            ads_core::queue::replay_queue(events.iter().map(QueueStreamEvent::as_log), owner_at)
                .unwrap();
        assert_eq!(replayed.policy, Some(initial));
        let [task] = &replayed.tasks[..] else {
            panic!("expected one task");
        };
        assert_eq!(task.attempt_limit.get(), 3);
    }

    /// The run token as the UI receives it, read back from the launch URL.
    fn token_text(token: &Token) -> String {
        let url = token.launch_url("http://127.0.0.1:5173", 1);
        url.rsplit("token=").next().unwrap().to_owned()
    }
}
