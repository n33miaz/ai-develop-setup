use crate::{
    StorageError,
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const META_SQL: &str =
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID";

/// Version 2: append-only event log. Only adds objects; never edits version 1.
const EVENT_LOG_SQL: &str = r#"
CREATE TABLE cipher_key (
  key_id TEXT PRIMARY KEY CHECK (length(key_id) BETWEEN 1 AND 128),
  cipher TEXT NOT NULL CHECK (cipher = 'xchacha20poly1305'),
  check_nonce BLOB NOT NULL CHECK (length(check_nonce) = 24),
  key_check BLOB NOT NULL CHECK (length(key_check) = 16)
) STRICT, WITHOUT ROWID;
CREATE TABLE event (
  position INTEGER PRIMARY KEY AUTOINCREMENT CHECK (position >= 1),
  id TEXT NOT NULL UNIQUE CHECK (length(id) = 26),
  type TEXT NOT NULL,
  ts TEXT NOT NULL,
  stream TEXT NOT NULL,
  seq INTEGER NOT NULL CHECK (seq >= 1),
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('device', 'agent', 'daemon')),
  actor_id TEXT NOT NULL,
  cause TEXT CHECK (cause IS NULL OR length(cause) = 26),
  data_class TEXT NOT NULL CHECK (data_class IN ('public', 'internal', 'private', 'secret')),
  key_id TEXT NOT NULL REFERENCES cipher_key(key_id),
  nonce BLOB NOT NULL CHECK (length(nonce) = 24),
  data_ct BLOB NOT NULL CHECK (length(data_ct) >= 16),
  prev_chain BLOB NOT NULL CHECK (length(prev_chain) = 32),
  chain BLOB NOT NULL CHECK (length(chain) = 32),
  UNIQUE (stream, seq)
) STRICT;
CREATE TRIGGER cipher_key_no_update BEFORE UPDATE ON cipher_key
BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER cipher_key_no_delete BEFORE DELETE ON cipher_key
BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER cipher_key_no_replace BEFORE INSERT ON cipher_key
BEGIN
  SELECT RAISE(ABORT, 'append-only')
  WHERE EXISTS (SELECT 1 FROM cipher_key WHERE key_id = NEW.key_id);
END;
CREATE TRIGGER event_no_update BEFORE UPDATE ON event
BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER event_no_delete BEFORE DELETE ON event
BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER event_append_order BEFORE INSERT ON event
BEGIN
  SELECT RAISE(ABORT, 'append-only')
  WHERE EXISTS (SELECT 1 FROM event WHERE id = NEW.id OR position = NEW.position);
  SELECT RAISE(ABORT, 'seq')
  WHERE NEW.seq IS NOT coalesce((SELECT max(seq) FROM event WHERE stream = NEW.stream), 0) + 1;
  SELECT RAISE(ABORT, 'chain')
  WHERE NEW.position IS NOT coalesce((SELECT max(position) FROM event), 0) + 1
     OR NEW.prev_chain IS NOT coalesce(
          (SELECT chain FROM event ORDER BY position DESC LIMIT 1), zeroblob(32));
END;
"#;

/// Scripts applied in order; the script at index `n` takes version `n` to `n + 1`.
const STEPS: [&str; SCHEMA_VERSION as usize] = [META_SQL, EVENT_LOG_SQL];

/// Schema objects a file of `version` must hold, built from the same scripts.
pub(crate) fn expected_schema(version: i64) -> Result<Vec<SchemaRow>, StorageError> {
    let steps = usize::try_from(version)
        .ok()
        .and_then(|n| STEPS.get(..n))
        .ok_or(StorageError::UnsupportedSchema)?;
    let connection = Connection::open_in_memory()?;
    for step in steps {
        connection.execute_batch(step)?;
    }
    schema(&connection)
}

pub(crate) type SchemaRow = (String, String, String, Option<String>);

/// Every object in `sqlite_schema`, including auto indexes without SQL text.
pub(crate) fn schema(c: &Connection) -> Result<Vec<SchemaRow>, StorageError> {
    let mut statement =
        c.prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")?;
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn migrate(connection: &mut Connection) -> Result<(), StorageError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if !(0..=SCHEMA_VERSION).contains(&version) {
        return Err(StorageError::UnsupportedSchema);
    }
    if version < SCHEMA_VERSION {
        // Re-checked inside the write lock: another opener may have changed it.
        if schema(&tx)? != expected_schema(version)? {
            return Err(StorageError::UnsupportedSchema);
        }
        for step in &STEPS[version as usize..] {
            tx.execute_batch(step)?;
        }
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    tx.commit()?;
    Ok(())
}
