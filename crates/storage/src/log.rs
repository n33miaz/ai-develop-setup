//! Append-only event log: encrypted `data`, authenticated clear columns, keyed chain.
use crate::{
    AuditStore, StorageConfig, StorageError,
    cipher::{self, AssociatedData, ChainKey, Ciphertext, KeyId, KeyProvider},
};
use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonDataClassV1, CommonIdV1, CommonTimestampV1,
    CommonUlidV1, EventEnvelopeV1, EventEnvelopeV1Stream, EventEnvelopeV1Type,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Map, Value};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    path::Path,
};
use zeroize::Zeroizing;

/// Label leading the associated data of every event row; the row format follows it.
const ROW_LABEL: &[u8] = b"urn:ads:storage:event-row";
/// Label leading the chain MAC input; the row format follows it.
const CHAIN_LABEL: &[u8] = b"urn:ads:storage:chain-row";
/// Format written by this build. A later format adds a value, never edits this one.
const ROW_FORMAT: i64 = 1;
const CIPHER: &str = "xchacha20poly1305";
const GENESIS: [u8; 32] = [0; 32];
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
}

impl std::fmt::Debug for EventLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventLog { .. }")
    }
}

/// Confirms the provider's key for `id` matches the one registered with it.
fn check_registered(
    tx: &rusqlite::Connection,
    keys: &dyn KeyProvider,
    id: &KeyId,
) -> Result<bool, StorageError> {
    let stored: Option<(Vec<u8>, Vec<u8>)> = tx
        .query_row(
            "SELECT check_nonce, key_check FROM cipher_key WHERE key_id = ?1",
            [id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((nonce, bytes)) = stored else {
        return Ok(false);
    };
    let check = Ciphertext {
        key_id: KeyId::new(id.as_str())?,
        nonce: nonce
            .as_slice()
            .try_into()
            .map_err(|_| StorageError::Corrupt)?,
        bytes,
    };
    match cipher::verify_key_check(keys, &check) {
        Ok(()) => Ok(true),
        Err(cipher::CipherError::Authentication) => Err(StorageError::WrongKey),
        Err(error) => Err(error.into()),
    }
}

/// Refuses a provider whose `current()` and `by_id` disagree on the same id.
fn consistent(keys: &dyn KeyProvider) -> Result<(), StorageError> {
    if cipher::provider_is_consistent(keys)? {
        Ok(())
    } else {
        Err(StorageError::WrongKey)
    }
}

fn head_in(tx: &rusqlite::Connection, stream: &str) -> Result<i64, StorageError> {
    Ok(tx.query_row(
        "SELECT coalesce(max(seq), 0) FROM event WHERE stream = ?1",
        [stream],
        |r| r.get(0),
    )?)
}

impl EventLog {
    /// Opens the store and refuses a current key that differs from the registered
    /// one, or a provider whose `current()` and `by_id` disagree.
    pub fn open(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
    ) -> Result<Self, StorageError> {
        let store = AuditStore::open(path, config)?;
        consistent(&*keys)?;
        let (id, _) = keys.current()?;
        check_registered(&store.connection, &*keys, &id)?;
        Ok(Self {
            store,
            keys,
            config: config.clone(),
        })
    }

    /// Last `seq` of the stream, 0 when it has no events.
    pub fn head(&self, stream: &EventEnvelopeV1Stream) -> Result<u64, StorageError> {
        let head = head_in(&self.store.connection, stream.as_str())?;
        u64::try_from(head).map_err(|_| StorageError::Corrupt)
    }

    /// Appends only if `event.seq` is still the next of its stream (compare and set).
    ///
    /// Nothing is written unless it returns `Ok`. `Conflict`: the `seq` is no longer
    /// the next one; read `head` again and rebuild the event. `Busy`: another
    /// writer held the lock past `busy_timeout_ms`; retrying the same event is safe.
    /// `Constraint`: the database refused the row (repeated `id`, or a trigger);
    /// do not retry unchanged. `InvalidEvent`: `type` or stream family over its
    /// bound, or a gate event whose `action_id`/`approval_id` disagree with its
    /// stream. `TooLarge`: `data` over `max_event_bytes`. `WrongKey`: the provider
    /// disagrees with itself or with the registered key.
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
        let keys = &*self.keys;
        consistent(keys)?;
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if seq != head_in(&tx, stream)? + 1 {
            return Err(StorageError::Conflict);
        }
        let key_id = register_current(&tx, keys)?;
        let last: Option<(i64, Vec<u8>)> = tx
            .query_row(
                "SELECT position, chain FROM event ORDER BY position DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (position, prev_chain) = match last {
            Some((position, chain)) => (position + 1, chain),
            None => (1, GENESIS.to_vec()),
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
        let sealed =
            row.associated(|fields| cipher::encrypt(keys, &AssociatedData(fields), &data))?;
        if sealed.key_id != key_id {
            return Err(StorageError::Cipher);
        }
        row.nonce = sealed.nonce.to_vec();
        row.data_ct = sealed.bytes;
        let chain_key = cipher::chain_key(keys, &key_id)?;
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
        tx.commit()?;
        Ok(Position(position))
    }

    /// Events from `from_seq` on, decrypted one row at a time. The page stops at
    /// `limit` (capped by `read_page_limit`) or before the row whose ciphertext
    /// would pass `read_page_max_bytes`; the first row always fits.
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
                let ciphertext = row.take_ciphertext()?;
                if checked.insert(row.key_id.clone())
                    && !check_registered(&tx, &*self.keys, &ciphertext.key_id)?
                {
                    return Err(StorageError::Corrupt);
                }
                let data = row.associated(|fields| {
                    cipher::decrypt(&*self.keys, &AssociatedData(fields), &ciphertext)
                })?;
                drop(ciphertext);
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
    /// The chain is an HMAC under a key derived from each row's provider key, so a
    /// missing old key is `KeyUnavailable`. It does not decrypt `data`, and it cannot
    /// see a cut tail or an older copy of the whole file.
    pub fn verify_chain(&self) -> Result<u64, StorageError> {
        let tx = self.store.connection.unchecked_transaction()?;
        let mut statement = tx.prepare(&format!(
            "SELECT {ROW_COLUMNS} FROM event ORDER BY position"
        ))?;
        let mut rows = statement.query([])?;
        let mut chain_keys: HashMap<String, ChainKey> = HashMap::new();
        let mut previous = GENESIS.to_vec();
        let mut count: i64 = 0;
        while let Some(r) = rows.next()? {
            let row = Row::from_sql(r)?;
            row.known_format()?;
            count += 1;
            if !chain_keys.contains_key(&row.key_id) {
                let id = KeyId::new(&row.key_id).map_err(|_| StorageError::Corrupt)?;
                if !check_registered(&tx, &*self.keys, &id)? {
                    return Err(StorageError::Corrupt);
                }
                let key = cipher::chain_key(&*self.keys, &id)?;
                chain_keys.insert(row.key_id.clone(), key);
            }
            let key = &chain_keys[&row.key_id];
            if row.position != count
                || row.prev_chain != previous
                || !row.chained(|fields| key.verify(fields, &row.chain))
            {
                return Err(StorageError::ChainBroken {
                    position: row.position,
                });
            }
            previous = row.chain;
        }
        u64::try_from(count).map_err(|_| StorageError::Corrupt)
    }
}

/// Registers the current key's check on first use, inside the append transaction.
fn register_current(tx: &Transaction<'_>, keys: &dyn KeyProvider) -> Result<KeyId, StorageError> {
    let (id, _) = keys.current()?;
    if !check_registered(tx, keys, &id)? {
        let check = cipher::key_check(keys)?;
        if check.key_id != id {
            return Err(StorageError::Cipher);
        }
        tx.execute(
            "INSERT INTO cipher_key (key_id, cipher, check_nonce, key_check) VALUES (?1, ?2, ?3, ?4)",
            params![id.as_str(), CIPHER, check.nonce.to_vec(), check.bytes],
        )?;
    }
    Ok(id)
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
        let ct = row
            .associated(|f| cipher::encrypt(keys, &AssociatedData(f), data))
            .unwrap();
        row.nonce = ct.nonce.to_vec();
        row.data_ct = ct.bytes;
        let key = cipher::chain_key(keys, &KeyId::new(&row.key_id).unwrap()).unwrap();
        row.chain = row.chained(|f| key.mac(f)).to_vec();
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
