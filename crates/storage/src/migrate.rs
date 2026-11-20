use crate::{
    StorageError,
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

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

const GENESIS_SQL: &str = r#"
CREATE TABLE log_genesis (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  link BLOB NOT NULL CHECK (length(link) = 32)
) STRICT, WITHOUT ROWID;
CREATE TRIGGER log_genesis_no_update BEFORE UPDATE ON log_genesis
BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER log_genesis_no_delete BEFORE DELETE ON log_genesis
BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER log_genesis_no_replace BEFORE INSERT ON log_genesis
BEGIN
  SELECT RAISE(ABORT, 'append-only')
  WHERE EXISTS (SELECT 1 FROM log_genesis WHERE singleton = NEW.singleton);
END;
DROP TRIGGER event_append_order;
CREATE TRIGGER event_append_order BEFORE INSERT ON event
BEGIN
  SELECT RAISE(ABORT, 'append-only')
  WHERE EXISTS (SELECT 1 FROM event WHERE id = NEW.id OR position = NEW.position);
  SELECT RAISE(ABORT, 'seq')
  WHERE NEW.seq IS NOT coalesce((SELECT max(seq) FROM event WHERE stream = NEW.stream), 0) + 1;
  SELECT RAISE(ABORT, 'chain')
  WHERE NEW.position IS NOT coalesce((SELECT max(position) FROM event), 0) + 1
     OR NEW.prev_chain IS NOT coalesce(
          (SELECT chain FROM event ORDER BY position DESC LIMIT 1),
          (SELECT link FROM log_genesis WHERE singleton = 1));
END;
"#;

/// `(version, script)`: each script takes the file from the previous listed
/// version to `version`. Version 2 (a pre-release event log) is retired: its rows
/// cannot be re-sealed without the key, which opening never receives.
const STEPS: [(i64, &str); 3] = [(1, META_SQL), (3, EVENT_LOG_SQL), (4, GENESIS_SQL)];
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
    migrate_with(connection, &STEPS, || {
        let mut link = [0; 32];
        getrandom::fill(&mut link).map_err(|_| crate::cipher::CipherError::Random)?;
        Ok(link)
    })
}

fn migrate_with(
    connection: &mut Connection,
    steps: &[(i64, &'static str)],
    random_link: impl FnOnce() -> Result<[u8; 32], StorageError>,
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
        let first_link: Option<Vec<u8>> = tx
            .query_row("SELECT prev_chain FROM event WHERE position = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let link = match first_link {
            Some(link) => link,
            None => random_link()?.to_vec(),
        };
        tx.execute("INSERT INTO log_genesis VALUES (1, ?1)", [link])?;
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
        let path = crate::open::tests::test_directory().join("audit.db");
        let mut c = Connection::open(&path).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        c.execute_batch(META_SQL).unwrap();
        c.execute("INSERT INTO meta VALUES ('kept', x'01')", [])
            .unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        let before = schema(&c).unwrap();
        let broken = [
            (1, META_SQL),
            (3, EVENT_LOG_SQL),
            (4, "CREATE TABLE half (x); CREATE TABLE"),
        ];
        assert!(migrate_with(&mut c, &broken, || unreachable!()).is_err());
        drop(c);
        let mut c = Connection::open(&path).unwrap();
        assert_eq!(version(&c), 1);
        assert_eq!(schema(&c).unwrap(), before);
        assert_eq!(
            c.query_row("SELECT value FROM meta WHERE key = 'kept'", [], |r| r
                .get::<_, Vec<u8>>(0))
                .unwrap(),
            [1]
        );
        // The real steps still migrate the same file afterwards.
        migrate(&mut c).unwrap();
        assert_eq!(version(&c), SCHEMA_VERSION);
    }

    #[test]
    fn version_three_authenticated_chain_survives_migration_and_append() {
        use crate::cipher::{CipherError, KeyId, KeyProvider, SecretKey};
        use ads_contracts::{CommonUlidV1, EventEnvelopeV1};

        struct TestKeys;
        impl KeyProvider for TestKeys {
            fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
                Ok((KeyId::new("key-a")?, SecretKey::new(&[1; 32])))
            }

            fn by_id(&self, _: &KeyId) -> Result<SecretKey, CipherError> {
                Ok(SecretKey::new(&[1; 32]))
            }
        }

        let path = crate::open::tests::test_directory().join("audit.db");
        let config = crate::StorageConfig::defaults();
        let open = || crate::EventLog::open(&path, &config, Box::new(TestKeys)).unwrap();
        let event = |n| {
            let mut event: EventEnvelopeV1 = serde_json::from_str(include_str!(
                "../../../contracts/golden/event.envelope.v1/valid/01-base.json"
            ))
            .unwrap();
            event.seq = std::num::NonZeroU64::new(n).unwrap();
            event.id = CommonUlidV1::new(format!("01J{n:023}")).unwrap();
            event
        };

        drop(open());
        let c = Connection::open(&path).unwrap();
        let update_trigger: Option<String> = c
            .query_row(
                "SELECT (SELECT sql FROM sqlite_schema WHERE name = 'log_genesis_no_update')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        c.execute_batch(
            "DROP TRIGGER IF EXISTS log_genesis_no_update;
             UPDATE log_genesis SET link = zeroblob(32);",
        )
        .unwrap();
        if let Some(sql) = update_trigger {
            c.execute_batch(&sql).unwrap();
        }
        drop(c);

        let mut log = open();
        log.append(&event(1)).unwrap();
        log.append(&event(2)).unwrap();
        assert_eq!(log.verify_chain(), Ok(2));
        drop(log);

        let c = Connection::open(&path).unwrap();
        let old_trigger = expected_schema(3)
            .unwrap()
            .into_iter()
            .find(|(_, name, _, _)| name == "event_append_order")
            .unwrap()
            .3
            .unwrap();
        c.execute_batch("DROP TRIGGER event_append_order; DROP TABLE log_genesis;")
            .unwrap();
        c.execute_batch(&old_trigger).unwrap();
        c.pragma_update(None, "user_version", 3).unwrap();
        assert_eq!(schema(&c).unwrap(), expected_schema(3).unwrap());
        let first_link: Vec<u8> = c
            .query_row("SELECT prev_chain FROM event WHERE position = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(first_link, [0; 32]);
        drop(c);

        let mut log = open();
        assert_eq!(log.verify_chain(), Ok(2));
        let stored = log.read_stream(&event(1).stream, 1, 2).unwrap();
        assert_eq!(stored[0].event, event(1));
        assert_eq!(stored[1].event, event(2));
        assert_eq!(log.append(&event(3)).unwrap().get(), 3);
        assert_eq!(log.verify_chain(), Ok(3));
    }

    #[test]
    fn random_failure_leaves_empty_version_three_unchanged() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(META_SQL).unwrap();
        c.execute_batch(EVENT_LOG_SQL).unwrap();
        c.pragma_update(None, "user_version", 3).unwrap();
        let error = StorageError::from(crate::cipher::CipherError::Random);
        assert_eq!(migrate_with(&mut c, &STEPS, || Err(error)), Err(error));
        assert_eq!(version(&c), 3);
        assert_eq!(schema(&c).unwrap(), expected_schema(3).unwrap());
        migrate_with(&mut c, &STEPS, || Ok([7; 32])).unwrap();
        let link: Vec<u8> = c
            .query_row("SELECT link FROM log_genesis", [], |r| r.get(0))
            .unwrap();
        assert_eq!(link, [7; 32]);
        c.execute(
            "INSERT INTO cipher_key VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["key-a", "xchacha20poly1305", [0_u8; 24], [0_u8; 16]],
        )
        .unwrap();
        let error = c
            .execute(
                "INSERT INTO event VALUES (
                1, 1, '01J00000000000000000000001', 'urn:ads:event:task.noted:1',
                '2026-11-19T12:00:00Z', 'task/t1', 1, 'device', 'phone',
                NULL, 'private', NULL, NULL, 'key-a', zeroblob(24), zeroblob(16),
                zeroblob(32), zeroblob(32)
            )",
                [],
            )
            .unwrap_err();
        assert!(
            matches!(error, rusqlite::Error::SqliteFailure(_, Some(message)) if message == "chain")
        );
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
