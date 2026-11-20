//! Append-only event log: encrypted `data`, authenticated clear columns, keyed chain.
use crate::{
    AuditStore, StorageConfig, StorageError,
    anchor::{
        AnchorStore, COMMITMENT_VERSION, Commitment, GENESIS, POSITION, REGISTERED_KEYS,
        ReanchorPlan,
    },
    cipher::{self, AssociatedData, ChainKey, Ciphertext, KeyId, KeyProvider, SecretKey},
};
use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonDataClassV1, CommonIdV1, CommonTimestampV1,
    CommonUlidV1, EventEnvelopeV1, EventEnvelopeV1Stream, EventEnvelopeV1Type,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Map, Value};
use std::{collections::HashSet, num::NonZeroU64, path::Path};
use storage_sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Label leading the associated data of every event row; the row format follows it.
const ROW_LABEL: &[u8] = b"urn:ads:storage:event-row";
/// Label leading the chain MAC input; the row format follows it.
const CHAIN_LABEL: &[u8] = b"urn:ads:storage:chain-row";
/// Format written by this build. A later format adds a value, never edits this one.
const ROW_FORMAT: i64 = 1;
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
        if self.format == ROW_FORMAT {
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
        let data: Map<String, Value> =
            serde_json::from_slice(data).map_err(|_| StorageError::Corrupt)?;
        // Only a key holder can write a row whose clear ids disagree with its data.
        if gate_ids(&self.kind, &self.stream, &data)
            != Ok((self.action_id.clone(), self.approval_id.clone()))
        {
            return Err(StorageError::Tampered);
        }
        Ok(EventEnvelopeV1 {
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
        })
    }
}

/// Audit store plus the key provider that encrypts and decrypts event data.
pub struct EventLog {
    store: AuditStore,
    keys: Box<dyn KeyProvider + Send>,
    config: StorageConfig,
    anchor: Option<Box<dyn AnchorStore>>,
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

fn recovery_plan(
    tx: &rusqlite::Connection,
    keys: &dyn KeyProvider,
    anchor: &dyn AnchorStore,
) -> Result<ReanchorPlan, StorageError> {
    let previous = anchor.read()?;
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
        previous,
        proposed: commitment(tx)?,
    })
}

fn head_in(tx: &rusqlite::Connection, stream: &str) -> Result<i64, StorageError> {
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

impl EventLog {
    /// Checks the independent vault commitment and authenticates the full chain
    /// under one write lock. Verification processes all stored ciphertext.
    pub fn open(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
        anchor: Box<dyn AnchorStore>,
    ) -> Result<Self, StorageError> {
        let create = anchor.read()?.is_none();
        let mut log = Self::open_store(path, config, keys, create)?;
        let tx = log
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = anchor.read()?;
        let actual = commitment(&tx)?;
        match stored {
            Some(expected) if expected == actual => {}
            Some(_) => return Err(StorageError::AnchorMismatch),
            None if empty_log(&tx)? => anchor.store(&actual)?,
            None => return Err(StorageError::AnchorMissing),
        }
        let (id, _) = log.keys.current()?;
        check_registered(&tx, &*log.keys, &id)?;
        verify_in(&tx, &*log.keys)?;
        tx.commit()?;
        log.anchor = Some(anchor);
        Ok(log)
    }

    /// Prepares a recovery plan after verifying all links and registrations.
    pub fn plan_reanchor(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
        anchor: &dyn AnchorStore,
    ) -> Result<ReanchorPlan, StorageError> {
        anchor.read()?;
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
        anchor.read()?;
        let mut log = Self::open_store(path, config, keys, false)?;
        let tx = log
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if recovery_plan(&tx, &*log.keys, anchor)? != confirmed {
            return Err(StorageError::AnchorMismatch);
        }
        anchor.store(&confirmed.proposed)?;
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
    /// `Constraint`: the database refused the row (repeated `id`, or a trigger);
    /// do not retry unchanged. `InvalidEvent`: `type` or stream family over its
    /// bound, or a gate event whose `action_id`/`approval_id` disagree with its
    /// stream. `TooLarge`: `data` over `max_event_bytes`. `WrongKey`: the provider
    /// disagrees with itself or with the registered key, or tries to reuse a retired id.
    /// `ChainBroken { position: 1 }`: the log has no genesis for the first row.
    pub fn append(&mut self, event: &EventEnvelopeV1) -> Result<Position, StorageError> {
        let kind = event.r#type.as_str();
        let stream = event.stream.as_str();
        let family = stream.split_once('/').map_or(stream, |(family, _)| family);
        if kind.len() > MAX_TYPE_BYTES || family.len() > MAX_STREAM_FAMILY_BYTES {
            return Err(StorageError::InvalidEvent);
        }
        let (action_id, approval_id) = gate_ids(kind, stream, &event.data)?;
        let data = Zeroizing::new(
            serde_json::to_vec(&event.data).map_err(|_| StorageError::InvalidEvent)?,
        );
        if data.len() > self.config.max_event_bytes {
            return Err(StorageError::TooLarge);
        }
        let seq = i64::try_from(event.seq.get()).map_err(|_| StorageError::InvalidEvent)?;
        // One fetch of the key material, used for the key check, the cipher and
        // the chain alike, so a provider that changes mid-append cannot split them.
        let (key_id, key) = {
            let (id, current) = self.keys.current()?;
            let key = self.keys.by_id(&id)?;
            if !cipher::same_key(&current, &key) {
                return Err(StorageError::WrongKey);
            }
            (id, key)
        };
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(anchor) = &self.anchor {
            match anchor.read()? {
                Some(expected) if expected == commitment(&tx)? => {}
                Some(_) => return Err(StorageError::AnchorMismatch),
                None => return Err(StorageError::AnchorMissing),
            }
        }
        if seq != head_in(&tx, stream)? + 1 {
            return Err(StorageError::Conflict);
        }
        let last: Option<(i64, Vec<u8>, String)> = tx
            .query_row(
                "SELECT position, chain, key_id FROM event ORDER BY position DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let last_id = last.as_ref().map(|(_, _, id)| id.as_str());
        register(&tx, &key_id, &key, last_id)?;
        let (position, prev_chain) = match last {
            Some((position, chain, _)) => (position + 1, chain),
            None => (1, genesis(&tx)?),
        };
        let mut row = Row {
            position,
            format: ROW_FORMAT,
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
        let sealed = row.associated(|fields| {
            cipher::encrypt_with(&key_id, &key, &AssociatedData(fields), &data)
        })?;
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
        if let Some(anchor) = &self.anchor {
            // Publish under the SQLite writer lock. A failed SQLite commit leaves a
            // mismatch that requires owner recovery instead of accepting an older head.
            anchor.store(&commitment(&tx)?)?;
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
        let from = i64::try_from(from_seq).unwrap_or(i64::MAX);
        let limit = limit.min(self.config.read_page_limit);
        let budget = self.config.read_page_max_bytes;
        let tx = self.store.connection.unchecked_transaction()?;
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
                    check_registered(&tx, &*self.keys, &id)
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
                    ) if self.authenticates_with_another_id(&tx, &row, &mut ciphertext)? => {
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
        tx.finish()?;
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
        if row.position != count || row.prev_chain != previous || row.format != ROW_FORMAT {
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
mod tests {
    use super::*;
    use crate::cipher::{CipherError, SecretKey};

    struct TwoKeys(u8);
    impl KeyProvider for TwoKeys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new("key-a")?, SecretKey::new(&[self.0; 32])))
        }
        fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
            match id.as_str() {
                "key-a" => Ok(SecretKey::new(&[self.0; 32])),
                "key-b" => Ok(SecretKey::new(&[2; 32])),
                _ => Err(CipherError::KeyUnavailable),
            }
        }
    }

    fn row() -> Row {
        Row {
            position: 7,
            format: ROW_FORMAT,
            id: "01J00000000000000000000007".into(),
            kind: "urn:ads:event:action.started:1".into(),
            ts: "2026-11-19T12:00:00Z".into(),
            stream: "action/a1".into(),
            seq: 3,
            actor_kind: "device".into(),
            actor_id: "phone".into(),
            cause: Some("01J00000000000000000000006".into()),
            data_class: "private".into(),
            action_id: Some("a1".into()),
            approval_id: Some("ap1".into()),
            key_id: "key-a".into(),
            nonce: vec![],
            data_ct: vec![],
            prev_chain: vec![9; 32],
            chain: vec![],
        }
    }

    fn seal(row: &mut Row, keys: &dyn KeyProvider, data: &[u8]) {
        let id = KeyId::new(&row.key_id).unwrap();
        let key = keys.by_id(&id).unwrap();
        let ct = row
            .associated(|f| cipher::encrypt_with(&id, &key, &AssociatedData(f), data))
            .unwrap();
        row.nonce = ct.nonce.to_vec();
        row.data_ct = ct.bytes;
        let chain_key = ChainKey::derive(&key);
        row.chain = row.chained(|f| chain_key.mac(f)).to_vec();
    }

    fn insert_authenticated_rows(
        connection: &mut rusqlite::Connection,
        ids: &[&str],
        mut previous: Vec<u8>,
    ) {
        let keys = TwoKeys(1);
        let tx = connection.transaction().unwrap();
        for id in ["key-a", "key-b"] {
            let id = KeyId::new(id).unwrap();
            register(&tx, &id, &keys.by_id(&id).unwrap(), None).unwrap();
        }
        for (position, id) in (1_i64..).zip(ids) {
            let mut row = row();
            row.position = position;
            row.seq = position;
            row.id = format!("01J{position:023}");
            row.key_id = (*id).to_owned();
            row.prev_chain = previous;
            seal(
                &mut row,
                &keys,
                br#"{"action_id":"a1","approval_id":"ap1","plan_hash":"x"}"#,
            );
            tx.execute(
                &format!(
                    "INSERT INTO event ({ROW_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
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
            )
            .unwrap();
            previous = row.chain;
        }
        tx.commit().unwrap();
    }

    #[test]
    fn verify_chain_refuses_authenticated_return_to_an_older_key() {
        let path = crate::open::tests::test_directory().join("audit.db");
        let config = StorageConfig::defaults();
        drop(AuditStore::open(&path, &config).unwrap());
        let mut connection = rusqlite::Connection::open(&path).unwrap();
        let link = genesis(&connection).unwrap();
        insert_authenticated_rows(&mut connection, &["key-a", "key-a", "key-b", "key-a"], link);
        drop(connection);

        let log = EventLog::open_unanchored(&path, &config, Box::new(TwoKeys(1))).unwrap();
        assert_eq!(
            log.verify_chain(),
            Err(StorageError::ChainBroken { position: 4 })
        );
    }

    #[test]
    fn migration_refuses_version_three_key_reuse_without_changing_the_log() {
        let path = crate::open::tests::test_directory().join("audit.db");
        let config = StorageConfig::defaults();
        drop(AuditStore::open(&path, &config).unwrap());
        let mut connection = rusqlite::Connection::open(&path).unwrap();
        let old_schema = crate::migrate::expected_schema(3).unwrap();
        let old_trigger = old_schema
            .iter()
            .find(|(_, name, _, _)| name == "event_append_order")
            .unwrap()
            .3
            .as_deref()
            .unwrap();
        connection
            .execute_batch("DROP TRIGGER event_append_order; DROP TABLE log_genesis;")
            .unwrap();
        connection.execute_batch(old_trigger).unwrap();
        connection.pragma_update(None, "user_version", 3).unwrap();
        insert_authenticated_rows(&mut connection, &["key-a", "key-b", "key-a"], vec![0; 32]);
        drop(connection);
        let before = std::fs::read(&path).unwrap();

        assert_eq!(
            AuditStore::open(&path, &config).unwrap_err(),
            StorageError::LegacyKeyReuse
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    fn copy(r: &Row) -> Row {
        Row {
            nonce: r.nonce.clone(),
            data_ct: r.data_ct.clone(),
            chain: r.chain.clone(),
            ..row()
        }
    }

    fn open(r: &Row) -> Result<Zeroizing<Vec<u8>>, StorageError> {
        let mut r = copy_all(r);
        r.known_format()?;
        let ct = r.take_ciphertext()?;
        Ok(r.associated(|f| cipher::decrypt(&TwoKeys(1), &AssociatedData(f), &ct))?)
    }

    fn copy_all(r: &Row) -> Row {
        Row {
            position: r.position,
            format: r.format,
            id: r.id.clone(),
            kind: r.kind.clone(),
            ts: r.ts.clone(),
            stream: r.stream.clone(),
            seq: r.seq,
            actor_kind: r.actor_kind.clone(),
            actor_id: r.actor_id.clone(),
            cause: r.cause.clone(),
            data_class: r.data_class.clone(),
            action_id: r.action_id.clone(),
            approval_id: r.approval_id.clone(),
            key_id: r.key_id.clone(),
            nonce: r.nonce.clone(),
            data_ct: r.data_ct.clone(),
            prev_chain: r.prev_chain.clone(),
            chain: r.chain.clone(),
        }
    }

    fn chain_ok(r: &Row) -> bool {
        let key = cipher::chain_key(&TwoKeys(1), &KeyId::new("key-a").unwrap()).unwrap();
        r.chained(|f| key.verify(f, &r.chain))
    }

    #[test]
    fn every_clear_column_is_authenticated_and_chained() {
        let mut sealed = row();
        seal(&mut sealed, &TwoKeys(1), b"{}");
        assert_eq!(&**open(&sealed).unwrap(), b"{}");
        assert!(chain_ok(&sealed));
        let changes: Vec<fn(&mut Row)> = vec![
            |r| r.position += 1,
            |r| r.id = "01J00000000000000000000008".into(),
            |r| r.kind = "urn:ads:event:action.executed:1".into(),
            |r| r.ts = "2026-11-19T12:00:01Z".into(),
            |r| r.stream = "action/a2".into(),
            |r| r.seq += 1,
            |r| r.actor_kind = "agent".into(),
            |r| r.actor_id = "laptop".into(),
            |r| r.cause = None,
            |r| r.cause = Some(String::new()),
            |r| r.data_class = "public".into(),
            |r| r.action_id = Some("a2".into()),
            |r| r.action_id = None,
            |r| r.approval_id = Some("ap2".into()),
            |r| r.approval_id = None,
            |r| r.prev_chain = vec![8; 32],
            |r| r.key_id = "key-b".into(),
            |r| r.nonce[0] ^= 1,
            |r| r.data_ct[0] ^= 1,
        ];
        for (index, change) in changes.iter().enumerate() {
            let mut altered = copy(&sealed);
            change(&mut altered);
            assert_eq!(
                open(&altered).unwrap_err(),
                StorageError::Tampered,
                "{index}"
            );
            assert!(!chain_ok(&altered), "{index}");
        }
        // An unknown format is refused before decrypting, and breaks the chain.
        let mut other = copy(&sealed);
        other.format = 2;
        assert_eq!(open(&other).unwrap_err(), StorageError::UnsupportedSchema);
        assert!(!chain_ok(&other));
        // The format is authenticated too, not only checked before decrypting.
        let ct = copy(&sealed).take_ciphertext().unwrap();
        let decrypted = other.associated(|f| cipher::decrypt(&TwoKeys(1), &AssociatedData(f), &ct));
        assert_eq!(decrypted.unwrap_err(), CipherError::Authentication);
    }

    #[test]
    fn chain_value_depends_on_the_key_not_only_on_the_row() {
        let mut first = row();
        seal(&mut first, &TwoKeys(1), b"{}");
        let mut second = copy_all(&first);
        let key = cipher::chain_key(&TwoKeys(5), &KeyId::new("key-a").unwrap()).unwrap();
        second.chain = second.chained(|f| key.mac(f)).to_vec();
        // Same columns, same key id, other key bytes: a writer without the key
        // cannot compute the link.
        assert_ne!(first.chain, second.chain);
        assert!(!chain_ok(&second));
    }

    #[test]
    fn clear_ids_must_match_the_decrypted_data() {
        let data = br#"{"action_id":"a1","approval_id":"ap1","plan_hash":"x"}"#;
        let mut good = row();
        seal(&mut good, &TwoKeys(1), data);
        let plain = open(&good).unwrap();
        good.envelope(&plain).unwrap();
        // Sealed by a key holder with clear ids its data does not name.
        let mut liar = row();
        liar.approval_id = None;
        seal(&mut liar, &TwoKeys(1), data);
        let plain = open(&liar).unwrap();
        assert_eq!(liar.envelope(&plain).unwrap_err(), StorageError::Tampered);
    }

    #[test]
    fn gate_ids_come_from_data_and_match_the_stream() {
        let data = |v: serde_json::Value| match v {
            Value::Object(map) => map,
            _ => unreachable!(),
        };
        let planned = "urn:ads:event:action.planned:1";
        let started = "urn:ads:event:action.started:1";
        let granted = "urn:ads:event:approval.granted:2";
        let granted_v1 = "urn:ads:event:approval.granted:1";
        let noted = "urn:ads:event:task.noted:1";
        let ok = |kind, stream, v, a: Option<&str>, b: Option<&str>| {
            assert_eq!(
                gate_ids(kind, stream, &data(v)),
                Ok((a.map(String::from), b.map(String::from))),
                "{kind} {stream}"
            )
        };
        ok(
            planned,
            "action/a1",
            serde_json::json!({ "action_id": "a1" }),
            Some("a1"),
            None,
        );
        ok(
            started,
            "action/a1",
            serde_json::json!({ "action_id": "a1", "approval_id": null }),
            Some("a1"),
            None,
        );
        ok(
            started,
            "action/a1",
            serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
            Some("a1"),
            Some("ap1"),
        );
        ok(
            granted,
            "approval/ap1",
            serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
            Some("a1"),
            Some("ap1"),
        );
        ok(
            granted_v1,
            "approval/ap1",
            serde_json::json!({ "approval_id": "ap1" }),
            None,
            Some("ap1"),
        );
        ok(
            noted,
            "task/t1",
            serde_json::json!({ "action_id": "a1" }),
            None,
            None,
        );
        for (kind, stream, v) in [
            (
                planned,
                "action/a1",
                serde_json::json!({ "action_id": "a2" }),
            ),
            (planned, "action/a1", serde_json::json!({})),
            (planned, "action/a1", serde_json::json!({ "action_id": 1 })),
            (
                planned,
                "action/a1",
                serde_json::json!({ "action_id": "a b" }),
            ),
            (
                started,
                "action/a1",
                serde_json::json!({ "action_id": "a1", "approval_id": 7 }),
            ),
            (planned, "task/a1", serde_json::json!({ "action_id": "a1" })),
            (noted, "action/a1", serde_json::json!({ "action_id": "a1" })),
            (
                granted,
                "approval/ap1",
                serde_json::json!({ "approval_id": "ap2" }),
            ),
            (
                granted,
                "action/ap1",
                serde_json::json!({ "action_id": "ap1" }),
            ),
            (noted, "noslash", serde_json::json!({})),
        ] {
            assert_eq!(
                gate_ids(kind, stream, &data(v)),
                Err(StorageError::InvalidEvent),
                "{kind} {stream}"
            );
        }
    }
}
