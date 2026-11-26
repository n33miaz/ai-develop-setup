//! Append-only event log: encrypted `data`, authenticated clear columns, keyed chain.
use crate::{
    AuditStore, StorageConfig, StorageError,
    anchor::{
        AnchorStore, COMMITMENT_VERSION, Commitment, DatabaseId, GENESIS, POSITION,
        REGISTERED_KEYS, ReanchorPlan,
    },
    cipher::{self, AssociatedData, ChainKey, Ciphertext, KeyId, KeyProvider, SecretKey},
    config::VerifyOnAppend,
    migrate::{expected_schema, schema},
    open::SCHEMA_VERSION,
};
use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonDataClassV1, CommonIdV1, CommonTimestampV1,
    CommonUlidV1, EventEnvelopeV1, EventEnvelopeV1Stream, EventEnvelopeV1Type,
};
use ads_core::gate::{
    ActionPlanRecord, Event, GateHistory, GateJournal, Id, JournalError, Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, num::NonZeroU64, path::Path};
use zeroize::Zeroizing;

/// Label leading the associated data of every event row; the row format follows it.
const ROW_LABEL: &[u8] = b"urn:ads:storage:event-row";
/// Label leading the chain MAC input; the row format follows it.
const CHAIN_LABEL: &[u8] = b"urn:ads:storage:chain-row";
/// The original format remains in use for ordinary events.
const ROW_FORMAT: i64 = 1;
const GATE_ROW_FORMAT: i64 = 3;

#[derive(serde::Serialize, serde::Deserialize)]
struct GateData {
    event: Map<String, Value>,
    plan: Option<ActionPlanRecord>,
}

struct GateAppend<'a> {
    action_id: &'a Id,
    expected_events: usize,
    plan: Option<&'a ActionPlanRecord>,
}
const CIPHER: &str = "xchacha20poly1305";
/// Bound of the clear `type`, in bytes; the schema checks the same.
pub const MAX_TYPE_BYTES: usize = 128;
/// Bound of the stream family (before `/`); the contract already bounds the id to 64.
pub const MAX_STREAM_FAMILY_BYTES: usize = 32;

/// Place of an event in the whole log, starting at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position(i64);
impl Position {
    pub fn get(self) -> i64 {
        self.0
    }
}

/// Decrypted event and its place in the log.
pub struct StoredEvent {
    pub position: Position,
    pub event: EventEnvelopeV1,
}
impl std::fmt::Debug for StoredEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredEvent {{ position: {}, .. }}", self.position.0)
    }
}

/// One `event` row as stored.
struct Row {
    position: i64,
    format: i64,
    id: String,
    kind: String,
    ts: String,
    stream: String,
    seq: i64,
    actor_kind: String,
    actor_id: String,
    cause: Option<String>,
    data_class: String,
    action_id: Option<String>,
    approval_id: Option<String>,
    key_id: String,
    nonce: Vec<u8>,
    data_ct: Vec<u8>,
    prev_chain: Vec<u8>,
    chain: Vec<u8>,
}

const ROW_COLUMNS: &str = "position, format, id, type, ts, stream, seq, actor_kind, actor_id, \
     cause, data_class, action_id, approval_id, key_id, nonce, data_ct, prev_chain, chain";
/// Index of `length(data_ct)`, selected after `ROW_COLUMNS` by `read_stream`.
const SIZE_COLUMN: usize = 18;

/// `action_id` and `approval_id` named by the `data` of a gate event, checked
/// against its stream (`contracts/README.md`, stream and entity). Other families
/// carry neither. An `action.*` or `approval.*` type on another stream, or the
/// reverse, is refused.
fn gate_ids(
    kind: &str,
    stream: &str,
    data: &Map<String, Value>,
) -> Result<(Option<String>, Option<String>), StorageError> {
    let gate = |family: &str| family == "action" || family == "approval";
    let type_family = kind
        .strip_prefix("urn:ads:event:")
        .and_then(|rest| rest.split('.').next())
        .ok_or(StorageError::InvalidEvent)?;
    let (stream_family, entity) = stream.split_once('/').ok_or(StorageError::InvalidEvent)?;
    if (gate(type_family) || gate(stream_family)) && type_family != stream_family {
        return Err(StorageError::InvalidEvent);
    }
    crate::queue::check_binding(type_family, stream_family, entity, data)?;
    let id = |name: &str| match data.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if CommonIdV1::new(text.as_str()).is_ok() => {
            Ok(Some(text.clone()))
        }
        Some(_) => Err(StorageError::InvalidEvent),
    };
    let own = |name: &str| match id(name)? {
        Some(text) if text == entity => Ok(Some(text)),
        _ => Err(StorageError::InvalidEvent),
    };
    match stream_family {
        "action" => Ok((own("action_id")?, id("approval_id")?)),
        "approval" => Ok((id("action_id")?, own("approval_id")?)),
        _ => Ok((None, None)),
    }
}

impl Row {
    fn from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            position: r.get(0)?,
            format: r.get(1)?,
            id: r.get(2)?,
            kind: r.get(3)?,
            ts: r.get(4)?,
            stream: r.get(5)?,
            seq: r.get(6)?,
            actor_kind: r.get(7)?,
            actor_id: r.get(8)?,
            cause: r.get(9)?,
            data_class: r.get(10)?,
            action_id: r.get(11)?,
            approval_id: r.get(12)?,
            key_id: r.get(13)?,
            nonce: r.get(14)?,
            data_ct: r.get(15)?,
            prev_chain: r.get(16)?,
            chain: r.get(17)?,
        })
    }

    /// Refuses a row format this build does not know before using any of it.
    fn known_format(&self) -> Result<(), StorageError> {
        if matches!(self.format, ROW_FORMAT | GATE_ROW_FORMAT) {
            Ok(())
        } else {
            Err(StorageError::UnsupportedSchema)
        }
    }

    /// Every clear column except the key id, which the cipher binds itself.
    fn associated<R>(&self, use_fields: impl FnOnce(&[Option<&[u8]>]) -> R) -> R {
        let format = self.format.to_be_bytes();
        let position = self.position.to_be_bytes();
        let seq = self.seq.to_be_bytes();
        use_fields(&[
            Some(ROW_LABEL),
            Some(&format),
            Some(&position),
            Some(self.id.as_bytes()),
            Some(self.kind.as_bytes()),
            Some(self.ts.as_bytes()),
            Some(self.stream.as_bytes()),
            Some(&seq),
            Some(self.actor_kind.as_bytes()),
            Some(self.actor_id.as_bytes()),
            self.cause.as_deref().map(str::as_bytes),
            Some(self.data_class.as_bytes()),
            self.action_id.as_deref().map(str::as_bytes),
            self.approval_id.as_deref().map(str::as_bytes),
            Some(&self.prev_chain),
        ])
    }

    /// Every column, ciphertext included, for the chain MAC.
    fn chained<R>(&self, use_fields: impl FnOnce(&[Option<&[u8]>]) -> R) -> R {
        let format = self.format.to_be_bytes();
        let position = self.position.to_be_bytes();
        let seq = self.seq.to_be_bytes();
        use_fields(&[
            Some(CHAIN_LABEL),
            Some(&format),
            Some(&self.prev_chain),
            Some(&position),
            Some(self.id.as_bytes()),
            Some(self.kind.as_bytes()),
            Some(self.ts.as_bytes()),
            Some(self.stream.as_bytes()),
            Some(&seq),
            Some(self.actor_kind.as_bytes()),
            Some(self.actor_id.as_bytes()),
            self.cause.as_deref().map(str::as_bytes),
            Some(self.data_class.as_bytes()),
            self.action_id.as_deref().map(str::as_bytes),
            self.approval_id.as_deref().map(str::as_bytes),
            Some(self.key_id.as_bytes()),
            Some(&self.nonce),
            Some(&self.data_ct),
        ])
    }

    /// Moves the ciphertext out of the row instead of copying it.
    fn take_ciphertext(&mut self) -> Result<Ciphertext, StorageError> {
        Ok(Ciphertext {
            key_id: KeyId::new(&self.key_id).map_err(|_| StorageError::Corrupt)?,
            nonce: self
                .nonce
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::Corrupt)?,
            bytes: std::mem::take(&mut self.data_ct),
        })
    }

    fn envelope(&self, data: &[u8]) -> Result<EventEnvelopeV1, StorageError> {
        self.envelope_with_plan(data).map(|(event, _)| event)
    }

    fn envelope_with_plan(
        &self,
        data: &[u8],
    ) -> Result<(EventEnvelopeV1, Option<ActionPlanRecord>), StorageError> {
        let bad = |_| StorageError::Corrupt;
        let kind = match self.actor_kind.as_str() {
            "device" => CommonActorV1Kind::Device,
            "agent" => CommonActorV1Kind::Agent,
            "daemon" => CommonActorV1Kind::Daemon,
            _ => return Err(StorageError::Corrupt),
        };
        let data_class = match self.data_class.as_str() {
            "public" => CommonDataClassV1::Public,
            "internal" => CommonDataClassV1::Internal,
            "private" => CommonDataClassV1::Private,
            "secret" => CommonDataClassV1::Secret,
            _ => return Err(StorageError::Corrupt),
        };
        let (data, plan) = if self.format == GATE_ROW_FORMAT {
            let payload: GateData =
                serde_json::from_slice(data).map_err(|_| StorageError::Corrupt)?;
            (payload.event, payload.plan)
        } else {
            (
                serde_json::from_slice(data).map_err(|_| StorageError::Corrupt)?,
                None,
            )
        };
        // Only a key holder can write a row whose clear ids disagree with its data.
        if gate_ids(&self.kind, &self.stream, &data)
            != Ok((self.action_id.clone(), self.approval_id.clone()))
        {
            return Err(StorageError::Tampered);
        }
        let event = EventEnvelopeV1 {
            id: CommonUlidV1::new(self.id.as_str()).map_err(bad)?,
            r#type: EventEnvelopeV1Type::new(self.kind.as_str()).map_err(bad)?,
            ts: CommonTimestampV1::new(self.ts.as_str()).map_err(bad)?,
            stream: EventEnvelopeV1Stream::new(self.stream.as_str()).map_err(bad)?,
            seq: u64::try_from(self.seq)
                .ok()
                .and_then(NonZeroU64::new)
                .ok_or(StorageError::Corrupt)?,
            actor: CommonActorV1 {
                kind,
                id: CommonIdV1::new(self.actor_id.as_str()).map_err(bad)?,
            },
            cause: self
                .cause
                .as_deref()
                .map(CommonUlidV1::new)
                .transpose()
                .map_err(bad)?,
            data_class,
            data,
        };
        Ok((event, plan))
    }
}

/// Audit store plus the key provider that encrypts and decrypts event data.
pub struct EventLog {
    store: AuditStore,
    keys: Box<dyn KeyProvider + Send>,
    config: StorageConfig,
    anchor: Option<Box<dyn AnchorStore>>,
    verified_data_version: i64,
    /// Full chain authentications run by this handle, for the tests of their cost.
    #[cfg(test)]
    pub(crate) verifications: usize,
}

impl std::fmt::Debug for EventLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventLog { .. }")
    }
}

/// The `key_check` registered for `id`, if any.
fn registered(tx: &rusqlite::Connection, id: &KeyId) -> Result<Option<Ciphertext>, StorageError> {
    let stored: Option<(Vec<u8>, Vec<u8>)> = tx
        .query_row(
            "SELECT check_nonce, key_check FROM cipher_key WHERE key_id = ?1",
            [id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((nonce, bytes)) = stored else {
        return Ok(None);
    };
    Ok(Some(Ciphertext {
        key_id: KeyId::new(id.as_str())?,
        nonce: nonce
            .as_slice()
            .try_into()
            .map_err(|_| StorageError::Corrupt)?,
        bytes,
    }))
}

fn matches_check(key: &SecretKey, check: &Ciphertext) -> Result<(), StorageError> {
    match cipher::verify_key_check_with(key, check) {
        Ok(()) => Ok(()),
        Err(cipher::CipherError::Authentication) => Err(StorageError::WrongKey),
        Err(error) => Err(error.into()),
    }
}

/// Confirms the provider's key for `id` matches the one registered with it.
/// `WrongKey` or `KeyUnavailable` here may also mean a forged `cipher_key` row.
fn check_registered(
    tx: &rusqlite::Connection,
    keys: &dyn KeyProvider,
    id: &KeyId,
) -> Result<bool, StorageError> {
    let Some(check) = registered(tx, id)? else {
        return Ok(false);
    };
    matches_check(&keys.by_id(id)?, &check)?;
    Ok(true)
}

/// Refuses a provider whose `current()` and `by_id` disagree on the same id.
fn consistent(keys: &dyn KeyProvider) -> Result<(), StorageError> {
    if cipher::provider_is_consistent(keys)? {
        Ok(())
    } else {
        Err(StorageError::WrongKey)
    }
}

fn empty_log(tx: &rusqlite::Connection) -> Result<bool, StorageError> {
    Ok(tx.query_row(
        "SELECT NOT EXISTS (SELECT 1 FROM event) AND NOT EXISTS (SELECT 1 FROM cipher_key)",
        [],
        |row| row.get(0),
    )?)
}

fn commitment(tx: &rusqlite::Connection) -> Result<Commitment, StorageError> {
    let genesis = genesis(tx)?;
    let head: Option<(i64, Vec<u8>)> = tx
        .query_row(
            "SELECT position, chain FROM event ORDER BY position DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (position, chain) = head.unwrap_or_else(|| (0, genesis.clone()));
    let mut result = [0; 105];
    result[0] = COMMITMENT_VERSION;
    result[GENESIS].copy_from_slice(&genesis);
    result[POSITION].copy_from_slice(&position.to_be_bytes());
    result[41..73].copy_from_slice(
        <&[u8; 32]>::try_from(chain.as_slice()).map_err(|_| StorageError::Corrupt)?,
    );

    let mut hash = Sha256::new();
    let mut statement = tx
        .prepare("SELECT key_id, cipher, check_nonce, key_check FROM cipher_key ORDER BY key_id")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        for column in 0..4 {
            let value = match row.get_ref(column)? {
                rusqlite::types::ValueRef::Text(value) | rusqlite::types::ValueRef::Blob(value) => {
                    value
                }
                _ => return Err(StorageError::Corrupt),
            };
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value);
        }
    }
    result[REGISTERED_KEYS].copy_from_slice(&hash.finalize());
    Ok(result)
}

fn database_id(tx: &Transaction<'_>, actual: &Commitment) -> Result<DatabaseId, StorageError> {
    let genesis: DatabaseId = actual[GENESIS].try_into().expect("genesis is 32 bytes");
    if genesis != [0; 32] {
        return Ok(genesis);
    }
    // Version 3 logs retained the zero genesis. Their authenticated first link is stable.
    let first: Vec<u8> = tx
        .query_row("SELECT chain FROM event WHERE position = 1", [], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or(StorageError::AnchorMismatch)?;
    if first.len() != 32 {
        return Err(StorageError::Corrupt);
    }
    let mut hash = Sha256::new();
    hash.update(b"urn:ads:storage:legacy-database-id:1");
    hash.update(first);
    Ok(hash.finalize().into())
}

fn recovery_plan(
    tx: &Transaction<'_>,
    keys: &dyn KeyProvider,
    anchor: &dyn AnchorStore,
) -> Result<ReanchorPlan, StorageError> {
    check_log_schema(tx)?;
    let proposed = commitment(tx)?;
    let id = database_id(tx, &proposed)?;
    let mut previous = anchor.recovery_anchor(id)?;
    if previous.previous_id.is_none() && previous.commitment == Some(proposed) {
        previous.previous_id = Some(id);
    }
    if previous.previous_id == Some(id)
        && previous
            .previous_location
            .zip(previous.proposed_location)
            .is_some_and(|(old, new)| old != new)
        && previous.commitment != Some(proposed)
    {
        return Err(StorageError::AnchorMismatch);
    }
    verify_in(tx, keys)?;
    let unused: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM cipher_key AS k
         WHERE NOT EXISTS (SELECT 1 FROM event AS e WHERE e.key_id = k.key_id))",
        [],
        |row| row.get(0),
    )?;
    if unused {
        return Err(StorageError::AnchorMismatch);
    }
    Ok(ReanchorPlan {
        id,
        previous,
        proposed,
    })
}

pub(crate) fn head_in(tx: &rusqlite::Connection, stream: &str) -> Result<i64, StorageError> {
    Ok(tx.query_row(
        "SELECT coalesce(max(seq), 0) FROM event WHERE stream = ?1",
        [stream],
        |r| r.get(0),
    )?)
}

fn genesis(tx: &rusqlite::Connection) -> Result<Vec<u8>, StorageError> {
    tx.query_row(
        "SELECT link FROM log_genesis WHERE singleton = 1",
        [],
        |r| r.get(0),
    )
    .optional()?
    .ok_or(StorageError::ChainBroken { position: 1 })
    .and_then(|link: Vec<u8>| {
        if link.len() == 32 {
            Ok(link)
        } else {
            Err(StorageError::Corrupt)
        }
    })
}

fn data_version(tx: &Transaction<'_>) -> Result<i64, StorageError> {
    Ok(tx.pragma_query_value(None, "data_version", |row| row.get(0))?)
}

/// A row whose registered key this provider cannot supply is a key problem
/// (another process rotated to a key this one does not hold), not tampering.
fn changed_history(
    tx: &rusqlite::Connection,
    keys: &dyn KeyProvider,
    position: i64,
) -> StorageError {
    let key_id: Option<String> = match tx
        .query_row(
            "SELECT key_id FROM event WHERE position = ?1",
            [position],
            |r| r.get(0),
        )
        .optional()
    {
        Ok(key_id) => key_id,
        Err(error) => return error.into(),
    };
    let unknown = key_id
        .and_then(|id| KeyId::new(&id).ok())
        .map(|id| match registered(tx, &id) {
            Ok(Some(_)) => matches!(keys.by_id(&id), Err(cipher::CipherError::KeyUnavailable)),
            _ => false,
        })
        .unwrap_or(false);
    if unknown {
        StorageError::KeyUnavailable
    } else {
        StorageError::HistoryChanged { position }
    }
}

fn check_log_schema(tx: &Transaction<'_>) -> Result<(), StorageError> {
    if schema(tx)? != expected_schema(SCHEMA_VERSION)? {
        return Err(StorageError::UnsupportedSchema);
    }
    Ok(())
}

impl EventLog {
    pub fn busy_timeout_ms(&self) -> u32 {
        self.config.busy_timeout_ms()
    }

    /// Checks the independent vault commitment and authenticates the full chain
    /// under one write lock. Verification processes all stored ciphertext.
    pub fn open(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
        anchor: Box<dyn AnchorStore>,
    ) -> Result<Self, StorageError> {
        let create = anchor.can_create()?;
        let mut log = Self::open_store(path, config, keys, create)?;
        let tx = log
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = data_version(&tx)?;
        // `migrate` checked the schema in another transaction; this one covers
        // the window between them, under the lock the chain is verified in.
        check_log_schema(&tx)?;
        let actual = commitment(&tx)?;
        let database = database_id(&tx, &actual)?;
        let stored = anchor.read(database)?;
        let expected = match stored {
            Some(expected) => Some(expected),
            None => anchor.legacy()?,
        };
        match expected {
            Some(expected) if expected == actual => {}
            Some(_) => return Err(StorageError::AnchorMismatch),
            None if empty_log(&tx)? && anchor.can_create()? => {}
            None => return Err(StorageError::AnchorMissing),
        }
        let (id, _) = log.keys.current()?;
        check_registered(&tx, &*log.keys, &id)?;
        verify_in(&tx, &*log.keys)?;
        if stored.is_none() {
            anchor.store(database, &actual)?;
        }
        anchor.bind(database)?;
        tx.commit()?;
        log.anchor = Some(anchor);
        log.verified_data_version = version;
        Ok(log)
    }

    /// Prepares a recovery plan after verifying all links and registrations.
    pub fn plan_reanchor(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
        anchor: &dyn AnchorStore,
    ) -> Result<ReanchorPlan, StorageError> {
        anchor.can_create()?;
        let mut log = Self::open_store(path, config, keys, false)?;
        let tx = log
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        recovery_plan(&tx, &*log.keys, anchor)
    }

    /// Applies the plan explicitly confirmed by the owner after making a backup.
    pub fn reanchor(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
        anchor: &dyn AnchorStore,
        confirmed: ReanchorPlan,
    ) -> Result<(), StorageError> {
        anchor.can_create()?;
        let mut log = Self::open_store(path, config, keys, false)?;
        let tx = log
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if recovery_plan(&tx, &*log.keys, anchor)? != confirmed {
            return Err(StorageError::AnchorMismatch);
        }
        anchor.recover(&confirmed)?;
        tx.commit()?;
        Ok(())
    }

    /// Opens the store and refuses a current key that differs from the registered
    /// one, or a provider whose `current()` and `by_id` disagree.
    #[cfg(test)]
    pub(crate) fn open_unanchored(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
    ) -> Result<Self, StorageError> {
        let log = Self::open_store(path, config, keys, true)?;
        let (id, _) = log.keys.current()?;
        check_registered(&log.store.connection, &*log.keys, &id)?;
        Ok(log)
    }

    fn open_store(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
        create: bool,
    ) -> Result<Self, StorageError> {
        let store = if create {
            AuditStore::open(path, config)?
        } else {
            AuditStore::open_existing(path, config)?
        };
        consistent(&*keys)?;
        Ok(Self {
            store,
            keys,
            config: config.clone(),
            anchor: None,
            verified_data_version: 0,
            #[cfg(test)]
            verifications: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn set_commit_hook(&mut self, hook: Option<fn() -> bool>) {
        self.store.connection.commit_hook(hook).unwrap();
    }

    /// Last `seq` of the stream, 0 when it has no events.
    pub fn head(&self, stream: &EventEnvelopeV1Stream) -> Result<u64, StorageError> {
        let head = head_in(&self.store.connection, stream.as_str())?;
        u64::try_from(head).map_err(|_| StorageError::Corrupt)
    }

    /// Appends only if `event.seq` is still the next of its stream (compare and set).
    ///
    /// The database row is committed after the vault update. If SQLite fails
    /// after that update, the next open or append refuses the mismatch.
    /// `Conflict`: the `seq` is no longer
    /// the next one; read `head` again and rebuild the event. `Busy`: SQLite
    /// could not obtain a lock in time. Check the log against the vault before
    /// attempting another write.
    /// `AnchorUnavailable`: restore access to the vault before trying again.
    /// `AnchorMissing`: the vault entry is absent. Stop and follow recovery.
    /// `AnchorMismatch`: the vault and database differ. Stop and follow recovery.
    /// `HistoryChanged`: the chain failed the check this append ran (after an
    /// external commit, or on every append with `verify_on_append = "always"`).
    /// Stop and follow recovery. `UnsupportedSchema`: the schema changed under
    /// that same check; stop and check the database. `KeyUnavailable`: that check
    /// met a registered key this provider cannot supply (another process
    /// rotated); give this provider the key and reopen.
    /// `Constraint`: the database refused the row (repeated `id`, or a trigger);
    /// do not retry unchanged. `InvalidEvent`: `type` or stream family over its
    /// bound, a gate event whose `action_id`/`approval_id` disagree with its
    /// stream, or a queue event outside `queue/<data.queue_id>` (and back).
    /// `QueueFull`: the queue stream already holds `queue_max_events` events
    /// (one `queue.stopped:1` still enters); raise `queue_max_events` and reopen,
    /// or create another queue. `Full`: the disk is full; free space and retry.
    /// `TooLarge`: `data` over `max_event_bytes`. `WrongKey`: the provider
    /// disagrees with itself or with the registered key, or tries to reuse a retired id.
    /// `ChainBroken { position: 1 }`: the log has no genesis for the first row.
    pub fn append(&mut self, event: &EventEnvelopeV1) -> Result<Position, StorageError> {
        self.append_inner(event, None)
    }

    fn append_inner(
        &mut self,
        event: &EventEnvelopeV1,
        gate: Option<GateAppend<'_>>,
    ) -> Result<Position, StorageError> {
        let sealing = sealing(&*self.keys, &self.config, event, gate.as_ref())?;
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let database = if let Some(anchor) = &self.anchor {
            let always = gate.is_some() || self.config.verify_on_append == VerifyOnAppend::Always;
            if always {
                // Drop cached pages so every check below reads what the file
                // holds now, not what this connection last saw.
                tx.pragma_query_value(None, "shrink_memory", |_| Ok(()))
                    .optional()?;
            }
            let actual = commitment(&tx)?;
            let database = database_id(&tx, &actual)?;
            match anchor.read(database)? {
                Some(expected) if expected == actual => {}
                Some(_) => return Err(StorageError::AnchorMismatch),
                None => return Err(StorageError::AnchorMissing),
            }
            // This connection's own commits do not change its data_version, and
            // neither does a write made straight to the file's bytes: only
            // `Always` catches the latter before the anchor moves.
            let version = data_version(&tx)?;
            if always || version != self.verified_data_version {
                check_log_schema(&tx)?;
                #[cfg(test)]
                {
                    self.verifications += 1;
                }
                verify_in(&tx, &*self.keys).map_err(|error| match error {
                    StorageError::ChainBroken { position } => {
                        changed_history(&tx, &*self.keys, position)
                    }
                    other => other,
                })?;
                self.verified_data_version = version;
            }
            Some((anchor, database))
        } else {
            None
        };
        if gate.is_some() && self.anchor.is_none() {
            return Err(StorageError::AnchorUnavailable);
        }
        let position = insert_in(&tx, event, sealing, gate.as_ref(), &self.config)?;
        if let Some((anchor, database)) = database {
            // Publish under the SQLite writer lock. A failed SQLite commit leaves a
            // mismatch that requires owner recovery instead of accepting an older head.
            anchor.store(database, &commitment(&tx)?)?;
        }
        tx.commit()?;
        Ok(Position(position))
    }

    /// Events from `from_seq` on, decrypted one row at a time. The page stops at
    /// `limit` (capped by `read_page_limit`) or before the row whose ciphertext
    /// would pass `read_page_max_bytes`; the first row always fits. A `seq` missing
    /// inside the page is `ChainBroken` at the row after the gap. Invalid,
    /// unregistered, or confirmed substituted key ids also break the chain.
    pub fn read_stream(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        let tx = self.store.connection.unchecked_transaction()?;
        let events = self.read_stream_in(&tx, stream, from_seq, limit)?;
        tx.finish()?;
        Ok(events)
    }

    /// The ciphertext size of each row of `stream` from `from_seq` on, at most `limit`
    /// (capped by `read_page_limit`): what a reader can weigh before asking for the rows.
    pub fn stream_sizes(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<usize>, StorageError> {
        let tx = self.store.connection.unchecked_transaction()?;
        let limit = limit.min(self.config.read_page_limit);
        let sizes = stream_sizes_in(&tx, stream, from_seq, limit)?;
        tx.finish()?;
        Ok(sizes)
    }

    /// Runs `read` under the write lock, after the anchor, the schema and the whole chain
    /// were checked in the same transaction, so it never sees a cut tail or a broken chain.
    /// The lock keeps an append from publishing its anchor between the check and the read.
    pub(crate) fn read_anchored<T>(
        &self,
        read: impl FnOnce(&Self, &Transaction<'_>) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let anchor = self
            .anchor
            .as_ref()
            .ok_or(StorageError::AnchorUnavailable)?;
        let tx =
            Transaction::new_unchecked(&self.store.connection, TransactionBehavior::Immediate)?;
        anchored_in(&tx, &**anchor, &*self.keys)?;
        let value = read(self, &tx)?;
        tx.finish()?;
        Ok(value)
    }

    /// `read_stream` inside a transaction the caller holds.
    pub(crate) fn read_stream_in(
        &self,
        tx: &rusqlite::Connection,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        let from = i64::try_from(from_seq).unwrap_or(i64::MAX);
        let limit = limit.min(self.config.read_page_limit);
        let budget = self.config.read_page_max_bytes;
        let mut events = Vec::new();
        {
            let mut statement = tx.prepare(&format!(
                "SELECT {ROW_COLUMNS}, length(data_ct) FROM event \
                 WHERE stream = ?1 AND seq >= ?2 ORDER BY seq LIMIT ?3"
            ))?;
            let mut rows = statement.query(params![stream.as_str(), from, limit])?;
            let mut checked = HashSet::new();
            let mut used = 0usize;
            let mut expected = from.max(1);
            while let Some(r) = rows.next()? {
                // Checked before the row is copied into Rust or decrypted; SQLite
                // itself has already stepped onto it.
                let size: i64 = r.get(SIZE_COLUMN)?;
                let size = usize::try_from(size).map_err(|_| StorageError::Corrupt)?;
                if !events.is_empty() && used.saturating_add(size) > budget {
                    break;
                }
                used = used.saturating_add(size);
                let mut row = Row::from_sql(r)?;
                row.known_format()?;
                let broken = StorageError::ChainBroken {
                    position: row.position,
                };
                let id = KeyId::new(&row.key_id).map_err(|_| broken)?;
                let registration = if checked.insert(row.key_id.clone()) {
                    check_registered(tx, &*self.keys, &id)
                } else {
                    Ok(true)
                };
                if matches!(registration, Ok(false)) {
                    return Err(broken);
                }
                if matches!(registration, Err(StorageError::KeyUnavailable)) {
                    return Err(broken);
                }
                let mut ciphertext = row.take_ciphertext()?;
                let data = match registration.and_then(|_| {
                    row.associated(|fields| {
                        cipher::decrypt(&*self.keys, &AssociatedData(fields), &ciphertext)
                            .map_err(StorageError::from)
                    })
                }) {
                    Err(
                        StorageError::Tampered
                        | StorageError::KeyUnavailable
                        | StorageError::WrongKey,
                    ) if self.authenticates_with_another_id(tx, &row, &mut ciphertext)? => {
                        return Err(broken);
                    }
                    Err(StorageError::KeyUnavailable) => return Err(broken),
                    result => result?,
                };
                drop(ciphertext);
                if row.seq != expected {
                    return Err(StorageError::ChainBroken {
                        position: row.position,
                    });
                }
                expected += 1;
                events.push(StoredEvent {
                    position: Position(row.position),
                    event: row.envelope(&data)?,
                });
            }
        }
        Ok(events)
    }

    /// Walks the whole log; returns the row count or the first row whose link breaks.
    /// The chain is an HMAC under a key derived from each row's provider key. A row
    /// out of sequence, of unknown format, or whose key the provider does not serve
    /// or does not match its `key_check` is `ChainBroken` at that row: it cannot be
    /// told apart from a forged one, so old keys must stay in the vault. It does not
    /// decrypt `data`, and it cannot see a cut tail or an older copy of the file.
    pub fn verify_chain(&self) -> Result<u64, StorageError> {
        let tx = self.store.connection.unchecked_transaction()?;
        verify_in(&tx, &*self.keys)
    }
}

/// What an append fixes before it writes: the ids of the gate families, the data
/// as it is sealed, and one fetch of the key material.
struct Sealing {
    action_id: Option<String>,
    approval_id: Option<String>,
    data: Zeroizing<Vec<u8>>,
    seq: i64,
    key_id: KeyId,
    key: SecretKey,
}

fn sealing(
    keys: &dyn KeyProvider,
    config: &StorageConfig,
    event: &EventEnvelopeV1,
    gate: Option<&GateAppend<'_>>,
) -> Result<Sealing, StorageError> {
    let kind = event.r#type.as_str();
    let stream = event.stream.as_str();
    let family = stream.split_once('/').map_or(stream, |(family, _)| family);
    if kind.len() > MAX_TYPE_BYTES || family.len() > MAX_STREAM_FAMILY_BYTES {
        return Err(StorageError::InvalidEvent);
    }
    let (action_id, approval_id) = gate_ids(kind, stream, &event.data)?;
    let data = Zeroizing::new(
        match gate {
            Some(gate) => serde_json::to_vec(&GateData {
                event: event.data.clone(),
                plan: gate.plan.cloned(),
            }),
            None => serde_json::to_vec(&event.data),
        }
        .map_err(|_| StorageError::InvalidEvent)?,
    );
    if data.len() > config.max_event_bytes {
        return Err(StorageError::TooLarge);
    }
    let seq = i64::try_from(event.seq.get()).map_err(|_| StorageError::InvalidEvent)?;
    // One fetch of the key material, used for the key check, the cipher and
    // the chain alike, so a provider that changes mid-append cannot split them.
    let (key_id, key) = {
        let (id, current) = keys.current()?;
        let key = keys.by_id(&id)?;
        if !cipher::same_key(&current, &key) {
            return Err(StorageError::WrongKey);
        }
        (id, key)
    };
    Ok(Sealing {
        action_id,
        approval_id,
        data,
        seq,
        key_id,
        key,
    })
}

/// Checks the gate invariants and the stream sequence, then inserts the sealed row
/// after the current head. The caller holds the write lock and publishes the anchor.
fn insert_in(
    tx: &Transaction<'_>,
    event: &EventEnvelopeV1,
    sealing: Sealing,
    gate: Option<&GateAppend<'_>>,
    config: &StorageConfig,
) -> Result<i64, StorageError> {
    let (action_id, approval_id) = (&sealing.action_id, &sealing.approval_id);
    if let Some(gate) = gate {
        check_gate_append(
            tx,
            event,
            gate,
            action_id.as_deref(),
            approval_id.as_deref(),
        )?;
    } else if (action_id.is_some() || approval_id.is_some())
        && tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM event
             WHERE format = ?1 AND (action_id = ?2 OR approval_id = ?3))",
            params![GATE_ROW_FORMAT, action_id, approval_id],
            |r| r.get(0),
        )?
    {
        return Err(StorageError::InvalidEvent);
    }
    let format = if gate.is_some() {
        GATE_ROW_FORMAT
    } else {
        ROW_FORMAT
    };
    insert_row(tx, event, sealing, format, config)
}

/// Checks the stream sequence and the queue cap, then inserts the sealed row in `format`
/// after the current head. The caller checked what the format requires.
fn insert_row(
    tx: &Transaction<'_>,
    event: &EventEnvelopeV1,
    sealing: Sealing,
    format: i64,
    config: &StorageConfig,
) -> Result<i64, StorageError> {
    let Sealing {
        action_id,
        approval_id,
        data,
        seq,
        key_id,
        key,
    } = sealing;
    let kind = event.r#type.as_str();
    let stream = event.stream.as_str();
    if seq != head_in(tx, stream)? + 1 {
        return Err(StorageError::Conflict);
    }
    if !crate::queue::within_cap(tx, kind, stream, seq, config)? {
        return Err(StorageError::QueueFull);
    }
    let last: Option<(i64, Vec<u8>, String)> = tx
        .query_row(
            "SELECT position, chain, key_id FROM event ORDER BY position DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let last_id = last.as_ref().map(|(_, _, id)| id.as_str());
    register(tx, &key_id, &key, last_id)?;
    let (position, prev_chain) = match last {
        Some((position, chain, _)) => (position + 1, chain),
        None => (1, genesis(tx)?),
    };
    let mut row = Row {
        position,
        format,
        id: event.id.as_str().to_owned(),
        kind: kind.to_owned(),
        ts: event.ts.as_str().to_owned(),
        stream: stream.to_owned(),
        seq,
        actor_kind: event.actor.kind.as_str().to_owned(),
        actor_id: event.actor.id.as_str().to_owned(),
        cause: event.cause.as_ref().map(|c| c.as_str().to_owned()),
        data_class: event.data_class.as_str().to_owned(),
        action_id,
        approval_id,
        key_id: key_id.as_str().to_owned(),
        nonce: Vec::new(),
        data_ct: Vec::new(),
        prev_chain,
        chain: Vec::new(),
    };
    let sealed = row
        .associated(|fields| cipher::encrypt_with(&key_id, &key, &AssociatedData(fields), &data))?;
    row.nonce = sealed.nonce.to_vec();
    row.data_ct = sealed.bytes;
    let chain_key = ChainKey::derive(&key);
    row.chain = row.chained(|fields| chain_key.mac(fields)).to_vec();
    tx.execute(
        &format!(
            "INSERT INTO event ({ROW_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
             ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"
        ),
        params![
            row.position,
            row.format,
            row.id,
            row.kind,
            row.ts,
            row.stream,
            row.seq,
            row.actor_kind,
            row.actor_id,
            row.cause,
            row.data_class,
            row.action_id,
            row.approval_id,
            row.key_id,
            row.nonce,
            row.data_ct,
            row.prev_chain,
            row.chain,
        ],
    )?;
    Ok(position)
}

fn check_gate_append(
    tx: &Transaction<'_>,
    event: &EventEnvelopeV1,
    gate: &GateAppend<'_>,
    action_id: Option<&str>,
    approval_id: Option<&str>,
) -> Result<(), StorageError> {
    let id = gate.action_id.as_str();
    if action_id != Some(id) {
        return Err(StorageError::InvalidEvent);
    }
    let (count, journal_rows): (i64, i64) = tx.query_row(
        "SELECT count(*), count(CASE WHEN format = ?2 THEN 1 END)
         FROM event WHERE action_id = ?1",
        params![id, GATE_ROW_FORMAT],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if usize::try_from(count).ok() != Some(gate.expected_events) || count != journal_rows {
        return Err(StorageError::Conflict);
    }

    let kind = event.r#type.as_str();
    let first = matches!(
        kind,
        "urn:ads:event:action.planned:1" | "urn:ads:event:action.denied:1"
    );
    if first != gate.plan.is_some()
        || first != (count == 0)
        || gate
            .plan
            .is_some_and(|plan| &plan.fields().action_id != gate.action_id)
    {
        return Err(StorageError::InvalidEvent);
    }
    let start = "urn:ads:event:action.started:1";
    let occupied: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM event WHERE id = ?1
                        OR (?3 AND action_id = ?2 AND type = ?4))",
        params![event.id.as_str(), id, kind == start, start],
        |r| r.get(0),
    )?;
    if occupied {
        return Err(StorageError::Conflict);
    }
    if let Some(approval_id) = approval_id {
        let (exists, foreign): (bool, bool) = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM event WHERE approval_id = ?1),
                    EXISTS (SELECT 1 FROM event WHERE approval_id = ?1 AND action_id IS NOT ?2)",
            params![approval_id, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let requested = kind == "urn:ads:event:approval.requested:2";
        if foreign || requested == exists {
            return Err(StorageError::Conflict);
        }
    }
    Ok(())
}

fn journal_error(error: StorageError) -> JournalError {
    match error {
        StorageError::Conflict | StorageError::Constraint => JournalError::Conflict,
        StorageError::AnchorMismatch
        | StorageError::AnchorMissing
        | StorageError::DuplicateDatabaseIdentity
        | StorageError::NonCanonicalDatabase
        | StorageError::ChainBroken { .. }
        | StorageError::HistoryChanged { .. }
        | StorageError::Corrupt
        | StorageError::Tampered
        | StorageError::UnsupportedSchema
        | StorageError::InvalidEvent => JournalError::Integrity,
        StorageError::Busy => JournalError::Busy,
        _ => JournalError::Unavailable,
    }
}

/// One pending approval as [`EventLog::visit_pending_approvals`] hands it over.
pub enum Pending {
    History(GateHistory),
    /// A gate request whose history does not read (a row this core does not know, or more
    /// rows than a history holds); its `action_id` as stored.
    Unreadable(String),
}

/// What the visitor of [`EventLog::visit_pending_approvals`] does with an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visit {
    /// Counts toward the limit.
    Take,
    Skip,
    Stop,
}

/// Reads in the transaction of [`EventLog::visit_pending_approvals`].
pub struct PendingReads<'a> {
    log: &'a EventLog,
    tx: &'a Transaction<'a>,
}

impl PendingReads<'_> {
    /// [`EventLog::read_stream`] in the listing's transaction.
    pub fn read_stream(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        self.log.read_stream_in(self.tx, stream, from_seq, limit)
    }

    /// [`EventLog::stream_sizes`] in the listing's transaction.
    pub fn stream_sizes(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<usize>, StorageError> {
        let limit = limit.min(self.log.config.read_page_limit);
        stream_sizes_in(self.tx, stream, from_seq, limit)
    }
}

/// Errors of `read_gate` that belong to the one history, not to the log.
fn unreadable_history(error: &StorageError) -> bool {
    matches!(
        error,
        StorageError::Corrupt | StorageError::InvalidEvent | StorageError::UnsupportedSchema
    )
}

/// The gate history stored under `action_id`, or `None` when it does not read (an id that is
/// not one, a row this core does not know, or more rows than a history holds). The listing
/// and the closing of an unreadable request judge with this one predicate.
fn readable_gate(
    tx: &Transaction<'_>,
    keys: &dyn KeyProvider,
    action_id: &str,
) -> Result<Option<GateHistory>, StorageError> {
    let Ok(id) = Id::new(action_id) else {
        return Ok(None);
    };
    match read_gate(tx, keys, &id) {
        Ok(history) => Ok(history),
        Err(error) if unreadable_history(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

const REQUESTED: &str = "urn:ads:event:approval.requested:2";
const GRANTED: &str = "urn:ads:event:approval.granted:2";
/// The only decisions the closing of an unreadable request writes: it never grants.
const CLOSING_TYPES: [&str; 2] = [
    "urn:ads:event:approval.denied:2",
    "urn:ads:event:approval.expired:2",
];

/// What [`EventLog::closable_requests`] finds for one action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closable {
    /// No gate request was ever stored for the action.
    Missing,
    /// The gate history reads: its requests are answered through the gate, never closed.
    Readable,
    /// The history does not read; its gate requests with no decision yet, oldest first.
    Requests(Vec<OpenRequest>),
}

/// A gate request with no decision, read from its own row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub approval_id: String,
    pub plan_hash: String,
    /// The end of the request's validity; an expiry is recorded only from this instant on.
    pub expires_at: String,
    /// The `seq` the decision takes in `approval/<approval_id>`.
    pub next_seq: u64,
}

impl OpenRequest {
    /// Whether the validity is over at `ts`, as the gate counts it: from `expires_at` on.
    /// An unreadable instant is never over.
    pub fn expired_at(&self, ts: &str) -> bool {
        let instant = |text: &str| CommonTimestampV1::new(text).ok().map(Timestamp::from);
        match (instant(ts), instant(&self.expires_at)) {
            (Some(ts), Some(expires_at)) => !ts.is_before(&expires_at),
            _ => false,
        }
    }
}

/// The gate requests of `action_id` with no gate decision, oldest first, at most `limit`.
fn open_requests(
    tx: &Transaction<'_>,
    keys: &dyn KeyProvider,
    action_id: &str,
    limit: u32,
) -> Result<Vec<OpenRequest>, StorageError> {
    let mut statement = tx.prepare(&format!(
        "SELECT {ROW_COLUMNS} FROM event AS asked
         WHERE asked.action_id = ?1 AND asked.type = ?2 AND asked.format = ?3
           AND asked.approval_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM event AS decided
                           WHERE decided.approval_id = asked.approval_id
                             AND decided.format = ?3
                             AND decided.type IN (?4, ?5, ?6))
         ORDER BY asked.position LIMIT ?7"
    ))?;
    let mut rows = statement.query(params![
        action_id,
        REQUESTED,
        GATE_ROW_FORMAT,
        GRANTED,
        CLOSING_TYPES[0],
        CLOSING_TYPES[1],
        limit,
    ])?;
    let mut open = Vec::new();
    while let Some(record) = rows.next()? {
        let mut row = Row::from_sql(record)?;
        let approval_id = row.approval_id.clone().ok_or(StorageError::Corrupt)?;
        // The request row on its own: the history around it is what does not read.
        let ciphertext = row.take_ciphertext()?;
        let data =
            row.associated(|fields| cipher::decrypt(keys, &AssociatedData(fields), &ciphertext))?;
        let (event, _) = row.envelope_with_plan(&data)?;
        let field = |name: &str| {
            event
                .data
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(StorageError::Corrupt)
        };
        let (plan_hash, expires_at) = (field("plan_hash")?, field("expires_at")?);
        let head = head_in(tx, &format!("approval/{approval_id}"))?;
        open.push(OpenRequest {
            approval_id,
            plan_hash,
            expires_at,
            next_seq: u64::try_from(head + 1).map_err(|_| StorageError::Corrupt)?,
        });
    }
    Ok(open)
}

fn closable_in(
    tx: &Transaction<'_>,
    keys: &dyn KeyProvider,
    action_id: &str,
    limit: u32,
) -> Result<Closable, StorageError> {
    let asked: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM event
                        WHERE action_id = ?1 AND type = ?2 AND format = ?3)",
        params![action_id, REQUESTED, GATE_ROW_FORMAT],
        |r| r.get(0),
    )?;
    if !asked {
        return Ok(Closable::Missing);
    }
    if readable_gate(tx, keys, action_id)?.is_some() {
        return Ok(Closable::Readable);
    }
    open_requests(tx, keys, action_id, limit).map(Closable::Requests)
}

/// The ciphertext size of each row from `from_seq` on, in `seq` order, without copying or
/// decrypting any of them.
fn stream_sizes_in(
    tx: &rusqlite::Connection,
    stream: &EventEnvelopeV1Stream,
    from_seq: u64,
    limit: u32,
) -> Result<Vec<usize>, StorageError> {
    let from = i64::try_from(from_seq).unwrap_or(i64::MAX);
    let mut statement = tx.prepare(
        "SELECT length(data_ct) FROM event WHERE stream = ?1 AND seq >= ?2 ORDER BY seq LIMIT ?3",
    )?;
    let mut rows = statement.query(params![stream.as_str(), from, limit])?;
    let mut sizes = Vec::new();
    while let Some(row) = rows.next()? {
        let size: i64 = row.get(0)?;
        sizes.push(usize::try_from(size).map_err(|_| StorageError::Corrupt)?);
    }
    Ok(sizes)
}

impl EventLog {
    /// The histories of the actions whose approval was requested and has no decision yet
    /// (granted, denied or expired) and that `keep` accepts, oldest request first, at most
    /// `limit` (capped by `read_page_limit`). Collects [`Self::visit_pending_approvals`]; an
    /// unreadable history is left out.
    pub fn pending_approvals(
        &self,
        limit: u32,
        keep: &mut dyn FnMut(&GateHistory) -> bool,
    ) -> Result<Vec<GateHistory>, StorageError> {
        let mut kept = Vec::new();
        self.visit_pending_approvals(limit, &mut |entry, _, full| match entry {
            _ if full => Visit::Stop,
            Pending::History(history) if keep(&history) => {
                kept.push(history);
                Visit::Take
            }
            _ => Visit::Skip,
        })?;
        Ok(kept)
    }

    /// Hands the pending approvals to `visit` one at a time, oldest request first, until it
    /// answers [`Visit::Stop`]. After `limit` entries are taken (capped by `read_page_limit`),
    /// the visitor receives `full = true` and can look for another eligible entry. Only gate
    /// rows count: a request or a decision of another row format neither lists nor hides an
    /// action. Each history is read only when its turn comes. Skipped histories do not count
    /// toward the page limit. `reads` serves other streams in the same transaction. A gate history that
    /// does not read comes as [`Pending::Unreadable`], once per action, and the walk goes on.
    /// Like `read_queue`, it runs under the write lock after the anchor and the whole chain were
    /// checked once. A validity that is over without an `approval.expired` still counts as
    /// waiting: the caller judges the clock.
    pub fn visit_pending_approvals(
        &self,
        limit: u32,
        visit: &mut dyn FnMut(Pending, &PendingReads<'_>, bool) -> Visit,
    ) -> Result<(), StorageError> {
        let limit = usize::try_from(limit.min(self.config.read_page_limit)).unwrap_or(usize::MAX);
        self.read_anchored(|log, tx| {
            let mut statement = tx.prepare(
                "WITH pending AS NOT MATERIALIZED (
                     SELECT asked.action_id, asked.position FROM event AS asked
                     WHERE asked.type = ?1 AND asked.format = ?5 AND asked.action_id IS NOT NULL
                       AND NOT EXISTS (SELECT 1 FROM event AS decided
                                       WHERE decided.approval_id = asked.approval_id
                                         AND decided.format = ?5
                                         AND decided.type IN (?2, ?3, ?4))
                 )
                 SELECT candidate.action_id FROM pending AS candidate
                 WHERE candidate.position = (
                     SELECT earlier.position FROM pending AS earlier
                     WHERE earlier.action_id = candidate.action_id
                     ORDER BY earlier.position LIMIT 1
                 )
                 ORDER BY candidate.position",
            )?;
            let mut rows = statement.query(params![
                "urn:ads:event:approval.requested:2",
                "urn:ads:event:approval.granted:2",
                "urn:ads:event:approval.denied:2",
                "urn:ads:event:approval.expired:2",
                GATE_ROW_FORMAT,
            ])?;
            let reads = PendingReads { log, tx };
            let mut taken = 0;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let entry = match readable_gate(tx, &*log.keys, &id)? {
                    Some(history) => Pending::History(history),
                    None => Pending::Unreadable(id),
                };
                let full = taken == limit;
                match visit(entry, &reads, full) {
                    Visit::Take if full => break,
                    Visit::Take => taken += 1,
                    Visit::Skip => {}
                    Visit::Stop => break,
                }
            }
            Ok(())
        })
    }

    /// What `approval.close` may close for `action_id`, at most `limit` requests, read under
    /// the write lock after the anchor and the whole chain were checked. Writes nothing.
    pub fn closable_requests(&self, action_id: &Id, limit: u32) -> Result<Closable, StorageError> {
        self.read_anchored(|log, tx| closable_in(tx, &*log.keys, action_id.as_str(), limit))
    }

    /// Writes `decision`, an `approval.denied:2` or `approval.expired:2` of a request of
    /// `action_id` whose gate history does not read, in the gate format, so that the request
    /// leaves the pending approvals. In the same transaction as the write it checks again that
    /// the history does not read and that the request is the action's and still open
    /// (`Conflict` otherwise); another type, other ids or another plan hash than the request's
    /// are `InvalidEvent`. For an expiry, the decision timestamp must be at or after the
    /// deadline. The caller uses the recorded closing timestamp, or the current time when
    /// resuming an expiry recorded before a remaining request's deadline.
    /// Nothing else reads that history, so no license comes out of it.
    pub fn close_unreadable_request(
        &mut self,
        action_id: &Id,
        decision: &EventEnvelopeV1,
    ) -> Result<Position, StorageError> {
        let anchor = self
            .anchor
            .as_ref()
            .ok_or(StorageError::AnchorUnavailable)?;
        if !CLOSING_TYPES.contains(&decision.r#type.as_str()) {
            return Err(StorageError::InvalidEvent);
        }
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        #[cfg(test)]
        {
            self.verifications += 1;
        }
        let (database, version) = anchored_in(&tx, &**anchor, &*self.keys)?;
        self.verified_data_version = version;
        let gate = GateAppend {
            action_id,
            expected_events: 0,
            plan: None,
        };
        let sealed = sealing(&*self.keys, &self.config, decision, Some(&gate))?;
        let Some(approval_id) = sealed.approval_id.clone() else {
            return Err(StorageError::InvalidEvent);
        };
        if sealed.action_id.as_deref() != Some(action_id.as_str()) {
            return Err(StorageError::InvalidEvent);
        }
        let open = match closable_in(&tx, &*self.keys, action_id.as_str(), u32::MAX)? {
            Closable::Requests(open) => open,
            Closable::Missing | Closable::Readable => return Err(StorageError::Conflict),
        };
        let Some(request) = open.iter().find(|open| open.approval_id == approval_id) else {
            return Err(StorageError::Conflict);
        };
        if decision.data.get("plan_hash").and_then(Value::as_str)
            != Some(request.plan_hash.as_str())
        {
            return Err(StorageError::InvalidEvent);
        }
        // An expiry before the deadline would record one that did not happen; deny stays open.
        if decision.r#type.as_str() == CLOSING_TYPES[1] && !request.expired_at(decision.ts.as_str())
        {
            return Err(StorageError::InvalidEvent);
        }
        let position = insert_row(&tx, decision, sealed, GATE_ROW_FORMAT, &self.config)?;
        anchor.store(database, &commitment(&tx)?)?;
        tx.commit()?;
        Ok(Position(position))
    }

    fn load_gate(&mut self, action_id: &Id) -> Result<Option<GateHistory>, StorageError> {
        let anchor = self
            .anchor
            .as_ref()
            .ok_or(StorageError::AnchorUnavailable)?;
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        #[cfg(test)]
        {
            self.verifications += 1;
        }
        anchored_in(&tx, &**anchor, &*self.keys)?;
        read_gate(&tx, &*self.keys, action_id)
    }

    /// Load, decision and append of one gate transition under one write lock, with one
    /// authentication of the whole chain before anything is read.
    fn transition_gate(
        &mut self,
        action_id: &Id,
        decide: &mut dyn FnMut(&GateHistory) -> Result<Event, JournalError>,
    ) -> Result<(), JournalError> {
        let anchor = self.anchor.as_ref().ok_or(JournalError::Unavailable)?;
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error(error.into()))?;
        #[cfg(test)]
        {
            self.verifications += 1;
        }
        let (database, version) =
            anchored_in(&tx, &**anchor, &*self.keys).map_err(journal_error)?;
        self.verified_data_version = version;
        let history = read_gate(&tx, &*self.keys, action_id)
            .map_err(journal_error)?
            .ok_or(JournalError::Missing)?;
        let event = decide(&history)?;
        let envelope: EventEnvelopeV1 =
            serde_json::from_value(event.to_json()).map_err(|_| JournalError::Integrity)?;
        let gate = GateAppend {
            action_id,
            expected_events: history.events.len(),
            plan: None,
        };
        let sealed =
            sealing(&*self.keys, &self.config, &envelope, Some(&gate)).map_err(journal_error)?;
        insert_in(&tx, &envelope, sealed, Some(&gate), &self.config).map_err(journal_error)?;
        let head = commitment(&tx).map_err(journal_error)?;
        anchor.store(database, &head).map_err(journal_error)?;
        tx.commit().map_err(|error| journal_error(error.into()))
    }

    fn append_gate(&mut self, event: &Event, gate: GateAppend<'_>) -> Result<(), JournalError> {
        let envelope =
            serde_json::from_value(event.to_json()).map_err(|_| JournalError::Integrity)?;
        self.append_inner(&envelope, Some(gate))
            .map_err(journal_error)?;
        Ok(())
    }
}

impl GateJournal for EventLog {
    fn load(&mut self, action_id: &Id) -> Result<GateHistory, JournalError> {
        self.load_gate(action_id)
            .map_err(journal_error)?
            .ok_or(JournalError::Missing)
    }

    fn create(&mut self, plan: &ActionPlanRecord, first: &Event) -> Result<(), JournalError> {
        self.append_gate(
            first,
            GateAppend {
                action_id: &plan.fields().action_id,
                expected_events: 0,
                plan: Some(plan),
            },
        )
    }

    fn append(
        &mut self,
        action_id: &Id,
        expected_events: usize,
        event: &Event,
    ) -> Result<(), JournalError> {
        self.append_gate(
            event,
            GateAppend {
                action_id,
                expected_events,
                plan: None,
            },
        )
    }

    fn transition(
        &mut self,
        action_id: &Id,
        decide: &mut dyn FnMut(&GateHistory) -> Result<Event, JournalError>,
    ) -> Result<(), JournalError> {
        self.transition_gate(action_id, decide)
    }
}

/// Drops cached pages, then checks the anchor, the schema and the whole chain under
/// the lock `tx` holds. Returns the database id and the data version it saw.
fn anchored_in(
    tx: &Transaction<'_>,
    anchor: &dyn AnchorStore,
    keys: &dyn KeyProvider,
) -> Result<(DatabaseId, i64), StorageError> {
    tx.pragma_query_value(None, "shrink_memory", |_| Ok(()))
        .optional()?;
    let actual = commitment(tx)?;
    let database = database_id(tx, &actual)?;
    let expected = anchor.read(database)?.ok_or(StorageError::AnchorMissing)?;
    if expected != actual {
        return Err(StorageError::AnchorMismatch);
    }
    let version = data_version(tx)?;
    check_log_schema(tx)?;
    verify_in(tx, keys)?;
    Ok((database, version))
}

/// The gate history of `action_id`, decrypted; `None` when the action has no rows.
fn read_gate(
    tx: &Transaction<'_>,
    keys: &dyn KeyProvider,
    action_id: &Id,
) -> Result<Option<GateHistory>, StorageError> {
    let mut statement = tx.prepare(&format!(
        "SELECT {ROW_COLUMNS} FROM event WHERE action_id = ?1 ORDER BY position LIMIT 6"
    ))?;
    let mut records = statement.query([action_id.as_str()])?;
    let mut events = Vec::new();
    let mut plan = None;
    while let Some(record) = records.next()? {
        let mut row = Row::from_sql(record)?;
        if row.format != GATE_ROW_FORMAT || events.len() == 5 {
            return Err(StorageError::Corrupt);
        }
        let ciphertext = row.take_ciphertext()?;
        let data =
            row.associated(|fields| cipher::decrypt(keys, &AssociatedData(fields), &ciphertext))?;
        let (event, recorded_plan) = row.envelope_with_plan(&data)?;
        if events.is_empty() {
            plan = Some(recorded_plan.ok_or(StorageError::Corrupt)?);
        } else if recorded_plan.is_some() {
            return Err(StorageError::Corrupt);
        }
        let json = serde_json::to_value(event).map_err(|_| StorageError::Corrupt)?;
        events.push(Event::from_json(json).map_err(|_| StorageError::Corrupt)?);
    }
    Ok(plan.map(|plan| GateHistory { plan, events }))
}

fn verify_in(tx: &rusqlite::Connection, keys: &dyn KeyProvider) -> Result<u64, StorageError> {
    let mut statement = tx.prepare(&format!(
        "SELECT {ROW_COLUMNS} FROM event ORDER BY position"
    ))?;
    let mut rows = statement.query([])?;
    let mut seen = HashSet::new();
    let mut active: Option<(String, ChainKey)> = None;
    let mut previous = genesis(tx)?;
    let mut count: i64 = 0;
    while let Some(r) = rows.next()? {
        let row = Row::from_sql(r)?;
        count += 1;
        let broken = StorageError::ChainBroken {
            position: row.position,
        };
        // Sequence and format first: neither needs a key.
        if row.position != count || row.prev_chain != previous || row.known_format().is_err() {
            return Err(broken);
        }
        if active.as_ref().is_none_or(|(id, _)| id != &row.key_id) {
            if !seen.insert(row.key_id.clone()) {
                return Err(broken);
            }
            let key = chain_key_of(tx, keys, &row.key_id)?.ok_or(broken)?;
            active = Some((row.key_id.clone(), key));
        }
        let (_, key) = active.as_ref().expect("active key for this row");
        if !row.chained(|fields| key.verify(fields, &row.chain)) {
            return Err(broken);
        }
        previous = row.chain;
    }
    u64::try_from(count).map_err(|_| StorageError::Corrupt)
}

impl EventLog {
    fn authenticates_with_another_id(
        &self,
        tx: &rusqlite::Connection,
        row: &Row,
        ciphertext: &mut Ciphertext,
    ) -> Result<bool, StorageError> {
        // Only run after key lookup or authentication fails. A successful alternate AAD proves
        // an id change while preserving Tampered for other damaged columns.
        let mut statement = tx
            .prepare("SELECT key_id FROM cipher_key WHERE key_id != ?1 ORDER BY key_id LIMIT ?2")?;
        let mut ids = statement.query(params![&row.key_id, self.config.read_key_candidates])?;
        while let Some(candidate) = ids.next()? {
            let candidate: String = candidate.get(0)?;
            let Ok(id) = KeyId::new(&candidate) else {
                continue;
            };
            let Some(check) = registered(tx, &id)? else {
                continue;
            };
            let Ok(key) = self.keys.by_id(&id) else {
                continue;
            };
            if matches_check(&key, &check).is_err() {
                continue;
            }
            ciphertext.key_id = id;
            if row
                .associated(|fields| {
                    cipher::decrypt_with(&key, &AssociatedData(fields), ciphertext)
                })
                .is_ok()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Chain key for `key_id`, or `None` when the id is malformed, unregistered,
/// unknown to the provider or registered with other key material.
fn chain_key_of(
    tx: &rusqlite::Connection,
    keys: &dyn KeyProvider,
    key_id: &str,
) -> Result<Option<ChainKey>, StorageError> {
    let Ok(id) = KeyId::new(key_id) else {
        return Ok(None);
    };
    let Some(check) = registered(tx, &id)? else {
        return Ok(None);
    };
    let key = match keys.by_id(&id) {
        Ok(key) => key,
        Err(cipher::CipherError::KeyUnavailable) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    match matches_check(&key, &check) {
        Ok(()) => Ok(Some(ChainKey::derive(&key))),
        Err(StorageError::WrongKey) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Registers the key's check on first use, inside the append transaction, or
/// confirms the key matches the one already registered.
fn register(
    tx: &Transaction<'_>,
    id: &KeyId,
    key: &SecretKey,
    last_id: Option<&str>,
) -> Result<(), StorageError> {
    match registered(tx, id)? {
        Some(_) if last_id.is_some_and(|last| last != id.as_str()) => Err(StorageError::WrongKey),
        Some(check) => matches_check(key, &check),
        None => {
            let check = cipher::key_check_with(id, key)?;
            tx.execute(
                "INSERT INTO cipher_key (key_id, cipher, check_nonce, key_check) VALUES (?1, ?2, ?3, ?4)",
                params![id.as_str(), CIPHER, check.nonce.to_vec(), check.bytes],
            )?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
