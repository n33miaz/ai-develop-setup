use crate::{
    StorageError,
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const META_SQL: &str =
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID";

/// Version 3: append-only event log, row format 1. Only adds objects to version 1.
const EVENT_LOG_SQL: &str = r#"
CREATE TABLE cipher_key (
  key_id TEXT PRIMARY KEY CHECK (length(key_id) BETWEEN 1 AND 128),
  cipher TEXT NOT NULL CHECK (cipher = 'xchacha20poly1305'),
  check_nonce BLOB NOT NULL CHECK (length(check_nonce) = 24),
  key_check BLOB NOT NULL CHECK (length(key_check) = 16)
) STRICT, WITHOUT ROWID;
CREATE TABLE event (
  position INTEGER PRIMARY KEY AUTOINCREMENT CHECK (position >= 1),
  format INTEGER NOT NULL CHECK (format >= 1),
  id TEXT NOT NULL UNIQUE CHECK (length(id) = 26),
  type TEXT NOT NULL CHECK (length(type) BETWEEN 1 AND 128),
  ts TEXT NOT NULL CHECK (length(ts) BETWEEN 20 AND 27),
  stream TEXT NOT NULL CHECK (length(stream) BETWEEN 3 AND 97),
  seq INTEGER NOT NULL CHECK (seq >= 1),
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('device', 'agent', 'daemon')),
  actor_id TEXT NOT NULL CHECK (length(actor_id) BETWEEN 1 AND 64),
  cause TEXT CHECK (cause IS NULL OR length(cause) = 26),
  data_class TEXT NOT NULL CHECK (data_class IN ('public', 'internal', 'private', 'secret')),
  action_id TEXT CHECK (action_id IS NULL OR length(action_id) BETWEEN 1 AND 64),
  approval_id TEXT CHECK (approval_id IS NULL OR length(approval_id) BETWEEN 1 AND 64),
  key_id TEXT NOT NULL REFERENCES cipher_key(key_id),
  nonce BLOB NOT NULL CHECK (length(nonce) = 24),
  data_ct BLOB NOT NULL CHECK (length(data_ct) BETWEEN 16 AND 16777232),
  prev_chain BLOB NOT NULL CHECK (length(prev_chain) = 32),
  chain BLOB NOT NULL CHECK (length(chain) = 32),
  UNIQUE (stream, seq),
  CHECK (substr(stream, 1, 7) <> 'action/' OR stream IS 'action/' || action_id),
  CHECK (substr(stream, 1, 9) <> 'approval/' OR stream IS 'approval/' || approval_id),
  CHECK (substr(stream, 1, 7) = 'action/' OR substr(stream, 1, 9) = 'approval/'
         OR (action_id IS NULL AND approval_id IS NULL)),
  CHECK ((substr(type, 1, 21) = 'urn:ads:event:action.') = (substr(stream, 1, 7) = 'action/')),
  CHECK ((substr(type, 1, 23) = 'urn:ads:event:approval.') = (substr(stream, 1, 9) = 'approval/'))
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

/// `(version, script)`: each script takes the file from the previous listed
/// version to `version`. Version 2 (a pre-release event log) is retired: its rows
/// cannot be re-sealed without the key, which opening never receives.
const STEPS: [(i64, &str); 2] = [(1, META_SQL), (3, EVENT_LOG_SQL)];
pub(crate) const RETIRED: [i64; 1] = [2];

/// Refuses versions this build never reads: negative, newer or retired.
pub(crate) fn readable(version: i64) -> Result<(), StorageError> {
    if version > SCHEMA_VERSION {
        Err(StorageError::SchemaTooNew)
    } else if RETIRED.contains(&version) {
        Err(StorageError::RetiredSchema)
    } else if version < 0 {
        Err(StorageError::UnsupportedSchema)
    } else {
        Ok(())
    }
}

fn scripts(steps: &[(i64, &'static str)], from: i64, to: i64) -> Vec<&'static str> {
    steps
        .iter()
        .filter(|(version, _)| (from + 1..=to).contains(version))
        .map(|(_, script)| *script)
        .collect()
}

/// Schema objects a file of `version` must hold, built from the same scripts.
pub(crate) fn expected_schema(version: i64) -> Result<Vec<SchemaRow>, StorageError> {
    expected_with(&STEPS, version)
}

fn expected_with(
    steps: &[(i64, &'static str)],
    version: i64,
) -> Result<Vec<SchemaRow>, StorageError> {
    readable(version)?;
    let connection = Connection::open_in_memory()?;
    for step in scripts(steps, 0, version) {
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
    migrate_with(connection, &STEPS)
}

fn migrate_with(
    connection: &mut Connection,
    steps: &[(i64, &'static str)],
) -> Result<(), StorageError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    // Compared inside the write lock for every version: another opener, or a
    // writer to the file, may have changed it since the inspection.
    if schema(&tx)? != expected_with(steps, version)? {
        return Err(StorageError::UnsupportedSchema);
    }
    if version < SCHEMA_VERSION {
        for step in scripts(steps, version, SCHEMA_VERSION) {
            tx.execute_batch(step)?;
        }
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(c: &Connection) -> i64 {
        c.pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn current_file_with_extra_object_is_refused_inside_the_lock() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c).unwrap();
        assert_eq!(version(&c), SCHEMA_VERSION);
        c.execute_batch("CREATE INDEX extra ON event(stream)")
            .unwrap();
        assert_eq!(migrate(&mut c), Err(StorageError::UnsupportedSchema));
    }

    #[test]
    fn failure_in_the_middle_leaves_version_one_intact() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(META_SQL).unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        let before = schema(&c).unwrap();
        // The second script creates a table, then fails.
        let broken: [(i64, &str); 2] = [(1, META_SQL), (3, "CREATE TABLE half (x); CREATE TABLE")];
        assert!(migrate_with(&mut c, &broken).is_err());
        assert_eq!(version(&c), 1);
        assert_eq!(schema(&c).unwrap(), before);
        // The real steps still migrate the same file afterwards.
        migrate(&mut c).unwrap();
        assert_eq!(version(&c), SCHEMA_VERSION);
    }

    #[test]
    fn retired_and_unknown_versions_have_no_expected_schema() {
        assert_eq!(expected_schema(2), Err(StorageError::RetiredSchema));
        assert_eq!(
            expected_schema(SCHEMA_VERSION + 1),
            Err(StorageError::SchemaTooNew)
        );
        assert_eq!(expected_schema(-1), Err(StorageError::UnsupportedSchema));
        assert_eq!(expected_schema(1).unwrap().len(), 1);
    }
}
