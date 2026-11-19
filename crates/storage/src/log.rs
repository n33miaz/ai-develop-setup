//! Append-only event log: encrypted `data`, authenticated clear columns, hash chain.
use crate::{
    AuditStore, StorageConfig, StorageError,
    cipher::{self, AssociatedData, Ciphertext, KeyId, KeyProvider},
};
use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonDataClassV1, CommonIdV1, CommonTimestampV1,
    CommonUlidV1, EventEnvelopeV1, EventEnvelopeV1Stream, EventEnvelopeV1Type,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, num::NonZeroU64, path::Path};
use zeroize::Zeroizing;

/// Versioned label leading the associated data of every event row.
const ROW_LABEL: &[u8] = b"urn:ads:storage:event:2";
/// Versioned label leading the chain hash input.
const CHAIN_LABEL: &[u8] = b"urn:ads:storage:chain:1";
const CIPHER: &str = "xchacha20poly1305";
const GENESIS: [u8; 32] = [0; 32];

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
    id: String,
    kind: String,
    ts: String,
    stream: String,
    seq: i64,
    actor_kind: String,
    actor_id: String,
    cause: Option<String>,
    data_class: String,
    key_id: String,
    nonce: Vec<u8>,
    data_ct: Vec<u8>,
    prev_chain: Vec<u8>,
    chain: Vec<u8>,
}

const ROW_COLUMNS: &str = "position, id, type, ts, stream, seq, actor_kind, actor_id, cause, \
     data_class, key_id, nonce, data_ct, prev_chain, chain";

impl Row {
    fn from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            position: r.get(0)?,
            id: r.get(1)?,
            kind: r.get(2)?,
            ts: r.get(3)?,
            stream: r.get(4)?,
            seq: r.get(5)?,
            actor_kind: r.get(6)?,
            actor_id: r.get(7)?,
            cause: r.get(8)?,
            data_class: r.get(9)?,
            key_id: r.get(10)?,
            nonce: r.get(11)?,
            data_ct: r.get(12)?,
            prev_chain: r.get(13)?,
            chain: r.get(14)?,
        })
    }

    /// Every clear column except the key id, which the cipher binds itself.
    fn associated<R>(&self, use_fields: impl FnOnce(&[Option<&[u8]>]) -> R) -> R {
        let position = self.position.to_be_bytes();
        let seq = self.seq.to_be_bytes();
        use_fields(&[
            Some(ROW_LABEL),
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
            Some(&self.prev_chain),
        ])
    }

    fn chain_hash(&self) -> [u8; 32] {
        let position = self.position.to_be_bytes();
        let seq = self.seq.to_be_bytes();
        let mut input = Vec::new();
        cipher::encode_fields(
            &mut input,
            &[
                Some(CHAIN_LABEL),
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
                Some(self.key_id.as_bytes()),
                Some(&self.nonce),
                Some(&self.data_ct),
            ],
        );
        Sha256::digest(&input).into()
    }

    fn ciphertext(&self) -> Result<Ciphertext, StorageError> {
        Ok(Ciphertext {
            key_id: KeyId::new(&self.key_id).map_err(|_| StorageError::Corrupt)?,
            nonce: self
                .nonce
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::Corrupt)?,
            bytes: self.data_ct.clone(),
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
            data: serde_json::from_slice(data).map_err(|_| StorageError::Corrupt)?,
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

fn head_in(tx: &rusqlite::Connection, stream: &str) -> Result<i64, StorageError> {
    Ok(tx.query_row(
        "SELECT coalesce(max(seq), 0) FROM event WHERE stream = ?1",
        [stream],
        |r| r.get(0),
    )?)
}

impl EventLog {
    /// Opens the store and refuses a current key that differs from the registered one.
    pub fn open(
        path: &Path,
        config: &StorageConfig,
        keys: Box<dyn KeyProvider + Send>,
    ) -> Result<Self, StorageError> {
        let store = AuditStore::open(path, config)?;
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
    pub fn append(&mut self, event: &EventEnvelopeV1) -> Result<Position, StorageError> {
        let data = Zeroizing::new(
            serde_json::to_vec(&event.data).map_err(|_| StorageError::InvalidEvent)?,
        );
        if data.len() > self.config.max_event_bytes {
            return Err(StorageError::TooLarge);
        }
        let seq = i64::try_from(event.seq.get()).map_err(|_| StorageError::InvalidEvent)?;
        let keys = &*self.keys;
        let tx = self
            .store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if seq != head_in(&tx, event.stream.as_str())? + 1 {
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
            id: event.id.as_str().to_owned(),
            kind: event.r#type.as_str().to_owned(),
            ts: event.ts.as_str().to_owned(),
            stream: event.stream.as_str().to_owned(),
            seq,
            actor_kind: event.actor.kind.as_str().to_owned(),
            actor_id: event.actor.id.as_str().to_owned(),
            cause: event.cause.as_ref().map(|c| c.as_str().to_owned()),
            data_class: event.data_class.as_str().to_owned(),
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
        row.chain = row.chain_hash().to_vec();
        tx.execute(
            &format!("INSERT INTO event ({ROW_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"),
            params![
                row.position,
                row.id,
                row.kind,
                row.ts,
                row.stream,
                row.seq,
                row.actor_kind,
                row.actor_id,
                row.cause,
                row.data_class,
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

    /// Up to `limit` events (capped by `read_page_limit`) from `from_seq` on, decrypted.
    pub fn read_stream(
        &self,
        stream: &EventEnvelopeV1Stream,
        from_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        let from = i64::try_from(from_seq).unwrap_or(i64::MAX);
        let limit = limit.min(self.config.read_page_limit);
        let tx = self.store.connection.unchecked_transaction()?;
        let rows = {
            let mut statement = tx.prepare(&format!(
                "SELECT {ROW_COLUMNS} FROM event WHERE stream = ?1 AND seq >= ?2 \
                 ORDER BY seq LIMIT ?3"
            ))?;
            statement
                .query_map(params![stream.as_str(), from, limit], Row::from_sql)?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut checked = HashSet::new();
        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let ciphertext = row.ciphertext()?;
            if checked.insert(row.key_id.clone())
                && !check_registered(&tx, &*self.keys, &ciphertext.key_id)?
            {
                return Err(StorageError::Corrupt);
            }
            let data = row.associated(|fields| {
                cipher::decrypt(&*self.keys, &AssociatedData(fields), &ciphertext)
            })?;
            events.push(StoredEvent {
                position: Position(row.position),
                event: row.envelope(&data)?,
            });
        }
        tx.finish()?;
        Ok(events)
    }

    /// Walks the whole log; returns the row count or the first row whose link breaks.
    /// Needs no key: the chain covers ciphertext and every clear column.
    pub fn verify_chain(&self) -> Result<u64, StorageError> {
        let tx = self.store.connection.unchecked_transaction()?;
        let mut statement = tx.prepare(&format!(
            "SELECT {ROW_COLUMNS} FROM event ORDER BY position"
        ))?;
        let mut rows = statement.query([])?;
        let mut previous = GENESIS.to_vec();
        let mut count: i64 = 0;
        while let Some(r) = rows.next()? {
            let row = Row::from_sql(r)?;
            count += 1;
            if row.position != count || row.prev_chain != previous || row.chain != row.chain_hash()
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

    struct TwoKeys;
    impl KeyProvider for TwoKeys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new("key-a")?, SecretKey::new(&[1; 32])))
        }
        fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
            match id.as_str() {
                "key-a" => Ok(SecretKey::new(&[1; 32])),
                "key-b" => Ok(SecretKey::new(&[2; 32])),
                _ => Err(CipherError::KeyUnavailable),
            }
        }
    }

    fn row() -> Row {
        Row {
            position: 7,
            id: "01J00000000000000000000007".into(),
            kind: "urn:ads:event:task.noted:1".into(),
            ts: "2026-11-19T12:00:00Z".into(),
            stream: "task/t1".into(),
            seq: 3,
            actor_kind: "device".into(),
            actor_id: "phone".into(),
            cause: Some("01J00000000000000000000006".into()),
            data_class: "private".into(),
            key_id: "key-a".into(),
            nonce: vec![],
            data_ct: vec![],
            prev_chain: vec![9; 32],
            chain: vec![],
        }
    }

    #[test]
    fn every_clear_column_is_authenticated_and_chained() {
        let mut sealed = row();
        let ct = sealed
            .associated(|f| cipher::encrypt(&TwoKeys, &AssociatedData(f), b"{}"))
            .unwrap();
        sealed.nonce = ct.nonce.to_vec();
        sealed.data_ct = ct.bytes;
        let open = |r: &Row| -> Result<Zeroizing<Vec<u8>>, StorageError> {
            let ct = r.ciphertext()?;
            Ok(r.associated(|f| cipher::decrypt(&TwoKeys, &AssociatedData(f), &ct))?)
        };
        assert_eq!(&**open(&sealed).unwrap(), b"{}");
        let changes: Vec<fn(&mut Row)> = vec![
            |r| r.position += 1,
            |r| r.id = "01J00000000000000000000008".into(),
            |r| r.kind = "urn:ads:event:task.other:1".into(),
            |r| r.ts = "2026-11-19T12:00:01Z".into(),
            |r| r.stream = "task/t2".into(),
            |r| r.seq += 1,
            |r| r.actor_kind = "agent".into(),
            |r| r.actor_id = "laptop".into(),
            |r| r.cause = None,
            |r| r.cause = Some(String::new()),
            |r| r.data_class = "public".into(),
            |r| r.prev_chain = vec![8; 32],
            |r| r.key_id = "key-b".into(),
            |r| r.nonce[0] ^= 1,
            |r| r.data_ct[0] ^= 1,
        ];
        let chain = sealed.chain_hash();
        for (index, change) in changes.iter().enumerate() {
            let mut altered = row();
            altered.nonce = sealed.nonce.clone();
            altered.data_ct = sealed.data_ct.clone();
            change(&mut altered);
            assert_eq!(
                open(&altered).unwrap_err(),
                StorageError::Tampered,
                "{index}"
            );
            assert_ne!(altered.chain_hash(), chain, "{index}");
        }
    }
}
