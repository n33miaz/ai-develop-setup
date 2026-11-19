use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonDataClassV1, CommonIdV1, CommonTimestampV1,
    CommonUlidV1, EventEnvelopeV1, EventEnvelopeV1Stream, EventEnvelopeV1Type,
};
use ads_storage::{
    AuditStore, EventLog, StorageConfig, StorageError,
    cipher::{CipherError, KeyId, KeyProvider, SecretKey},
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use rusqlite::Connection;
use std::{
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const SENTINEL: &str = "private-sentinel-5f2e9c1d";
const CLEAR_MARKER: &str = "clear-actor-marker-81b3";

fn fixture() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    assert!(root.is_absolute());
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    loop {
        let dir = root.join(format!(
            "event-log-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(dir.starts_with(&root));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir.join("audit.db"),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("fixture directory: {error}"),
        }
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

/// Current key plus older keys still available for reading.
struct Keys {
    current: (&'static str, u8),
    older: Vec<(&'static str, u8)>,
}

fn keys(id: &'static str, byte: u8) -> Box<Keys> {
    Box::new(Keys {
        current: (id, byte),
        older: vec![],
    })
}

impl KeyProvider for Keys {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        Ok((
            KeyId::new(self.current.0)?,
            SecretKey::new(&[self.current.1; 32]),
        ))
    }
    fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
        std::iter::once(&self.current)
            .chain(&self.older)
            .find(|(name, _)| *name == id.as_str())
            .map(|(_, byte)| SecretKey::new(&[*byte; 32]))
            .ok_or(CipherError::KeyUnavailable)
    }
}

fn open(path: &Path) -> EventLog {
    EventLog::open(path, &StorageConfig::defaults(), keys("key-a", 1)).unwrap()
}

fn stream(name: &str) -> EventEnvelopeV1Stream {
    EventEnvelopeV1Stream::new(name).unwrap()
}

fn event(stream_name: &str, seq: u64, n: u64, data: serde_json::Value) -> EventEnvelopeV1 {
    EventEnvelopeV1 {
        id: CommonUlidV1::new(format!("01J{n:023}")).unwrap(),
        r#type: EventEnvelopeV1Type::new("urn:ads:event:task.noted:1").unwrap(),
        ts: CommonTimestampV1::new("2026-11-19T12:00:00Z").unwrap(),
        stream: stream(stream_name),
        seq: NonZeroU64::new(seq).unwrap(),
        actor: CommonActorV1 {
            kind: CommonActorV1Kind::Device,
            id: CommonIdV1::new(CLEAR_MARKER).unwrap(),
        },
        cause: (n > 1).then(|| CommonUlidV1::new(format!("01J{:023}", n - 1)).unwrap()),
        data_class: CommonDataClassV1::Private,
        data: match data {
            serde_json::Value::Object(map) => map,
            _ => panic!("data must be an object"),
        },
    }
}

/// Three events of `task/t1`, the middle one carrying the sentinel.
fn three(log: &mut EventLog) -> Vec<EventEnvelopeV1> {
    let events: Vec<_> = (1..=3)
        .map(|n| {
            let text = if n == 2 { SENTINEL } else { "plain" };
            event("task/t1", n, n, serde_json::json!({ "text": text, "n": n }))
        })
        .collect();
    for (index, e) in events.iter().enumerate() {
        assert_eq!(log.append(e).unwrap().get(), index as i64 + 1);
    }
    events
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Bytes of the database and its `-wal` and `-shm`, by suffix, for files present.
fn files(path: &Path) -> Vec<(&'static str, Vec<u8>)> {
    ["", "-wal", "-shm"]
        .into_iter()
        .filter_map(|suffix| {
            std::fs::read(sibling(path, suffix))
                .ok()
                .map(|bytes| (suffix, bytes))
        })
        .collect()
}

fn assert_no_sentinel(path: &Path) {
    let all = files(path);
    for (suffix, bytes) in &all {
        assert!(
            !contains(bytes, SENTINEL.as_bytes()),
            "plaintext in audit.db{suffix}"
        );
    }
    // Positive control: the scan finds clear columns in the same files.
    assert!(
        all.iter()
            .any(|(_, bytes)| contains(bytes, CLEAR_MARKER.as_bytes()))
    );
}

#[test]
fn sentinel_never_reaches_database_wal_or_shm() {
    let path = fixture();
    let mut log = open(&path);
    let events = three(&mut log);
    let suffixes: Vec<_> = files(&path).into_iter().map(|(s, _)| s).collect();
    assert_eq!(suffixes, ["", "-wal", "-shm"]);
    assert_no_sentinel(&path);
    drop(log);
    assert_no_sentinel(&path);
    let log = open(&path);
    let read = log.read_stream(&stream("task/t1"), 1, 10).unwrap();
    assert_eq!(
        read.iter().map(|s| s.event.clone()).collect::<Vec<_>>(),
        events
    );
    assert_no_sentinel(&path);
    let diagnostic = format!("{read:?} {log:?}");
    assert!(!diagnostic.contains(SENTINEL));
    drop(log);
    assert_eq!(
        EventLog::open(&path, &StorageConfig::defaults(), keys("key-a", 2)).unwrap_err(),
        StorageError::WrongKey
    );
    // A new current key cannot read rows written under a different key-a.
    let rotated = Box::new(Keys {
        current: ("key-b", 3),
        older: vec![("key-a", 2)],
    });
    let log = EventLog::open(&path, &StorageConfig::defaults(), rotated).unwrap();
    assert_eq!(
        log.read_stream(&stream("task/t1"), 1, 10).unwrap_err(),
        StorageError::WrongKey
    );
}

#[test]
fn rotation_reads_old_rows_with_older_key() {
    let path = fixture();
    let mut log = open(&path);
    let first = three(&mut log);
    drop(log);
    let rotated = Box::new(Keys {
        current: ("key-b", 3),
        older: vec![("key-a", 1)],
    });
    let mut log = EventLog::open(&path, &StorageConfig::defaults(), rotated).unwrap();
    let fourth = event("task/t1", 4, 4, serde_json::json!({ "text": "after" }));
    log.append(&fourth).unwrap();
    let read: Vec<_> = log
        .read_stream(&stream("task/t1"), 1, 10)
        .unwrap()
        .into_iter()
        .map(|s| s.event)
        .collect();
    assert_eq!(read[..3], first[..]);
    assert_eq!(read[3], fourth);
    assert_eq!(log.verify_chain(), Ok(4));
}

/// Runs `change` on a raw connection with every append-only trigger dropped,
/// then recreates them with identical text so the schema check still passes.
fn tamper_with(path: &Path, change: impl FnOnce(&Connection)) {
    let c = Connection::open(path).unwrap();
    let triggers: Vec<(String, String)> = c
        .prepare("SELECT name, sql FROM sqlite_schema WHERE type = 'trigger'")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(triggers.len(), 6);
    for (name, _) in &triggers {
        c.execute_batch(&format!("DROP TRIGGER {name}")).unwrap();
    }
    change(&c);
    for (_, sql) in &triggers {
        c.execute_batch(sql).unwrap();
    }
}

/// `tamper_with` for one statement that must change exactly one row.
fn tamper(path: &Path, sql: &str, blob: Option<Vec<u8>>) {
    tamper_with(path, |c| {
        match blob {
            Some(bytes) => assert_eq!(c.execute(sql, [bytes]).unwrap(), 1),
            None => assert_eq!(c.execute(sql, []).unwrap(), 1),
        };
    });
}

fn data_ct(path: &Path, position: i64) -> Vec<u8> {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT data_ct FROM event WHERE position = ?1",
            [position],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn altered_clear_column_or_ciphertext_is_refused_on_read() {
    let cases: &[(&str, &str)] = &[
        (
            "id",
            "UPDATE event SET id = '01J99999999999999999999999' WHERE position = 2",
        ),
        (
            "type",
            "UPDATE event SET type = 'urn:ads:event:task.other:1' WHERE position = 2",
        ),
        (
            "ts",
            "UPDATE event SET ts = '2026-11-19T12:00:01Z' WHERE position = 2",
        ),
        (
            "stream",
            "UPDATE event SET stream = 'task/moved' WHERE position = 2",
        ),
        ("seq", "UPDATE event SET seq = 4 WHERE position = 3"),
        (
            "position",
            "UPDATE event SET position = 9 WHERE position = 3",
        ),
        (
            "actor_kind",
            "UPDATE event SET actor_kind = 'agent' WHERE position = 2",
        ),
        (
            "actor_id",
            "UPDATE event SET actor_id = 'someone-else' WHERE position = 2",
        ),
        (
            "cause set",
            "UPDATE event SET cause = '01J00000000000000000000009' WHERE position = 1",
        ),
        (
            "cause cleared",
            "UPDATE event SET cause = NULL WHERE position = 2",
        ),
        (
            "data_class",
            "UPDATE event SET data_class = 'public' WHERE position = 2",
        ),
        (
            "prev_chain",
            "UPDATE event SET prev_chain = zeroblob(32) WHERE position = 2",
        ),
        (
            "key_id",
            "UPDATE event SET key_id = 'key-b' WHERE position = 2",
        ),
        (
            "nonce",
            "UPDATE event SET nonce = zeroblob(24) WHERE position = 2",
        ),
    ];
    for (column, sql) in cases {
        let path = fixture();
        let mut log = open(&path);
        three(&mut log);
        // A row of another stream, and a second registered key, for the swaps.
        log.append(&event("task/t2", 1, 4, serde_json::json!({ "x": 1 })))
            .unwrap();
        drop(log);
        let rotated = Box::new(Keys {
            current: ("key-b", 5),
            older: vec![("key-a", 1)],
        });
        let mut log = EventLog::open(&path, &StorageConfig::defaults(), rotated).unwrap();
        log.append(&event("task/t3", 1, 5, serde_json::json!({ "x": 2 })))
            .unwrap();
        drop(log);
        tamper(&path, sql, None);
        let read = if *column == "stream" {
            "task/moved"
        } else {
            "task/t1"
        };
        let rotated = Box::new(Keys {
            current: ("key-b", 5),
            older: vec![("key-a", 1)],
        });
        let log = EventLog::open(&path, &StorageConfig::defaults(), rotated).unwrap();
        assert_eq!(
            log.read_stream(&stream(read), 1, 10).unwrap_err(),
            StorageError::Tampered,
            "{column}"
        );
        assert!(
            matches!(log.verify_chain(), Err(StorageError::ChainBroken { .. })),
            "{column}"
        );
    }
    for flip in [0usize, 20] {
        let path = fixture();
        let mut log = open(&path);
        three(&mut log);
        drop(log);
        let mut bytes = data_ct(&path, 2);
        let at = flip.min(bytes.len() - 1);
        bytes[at] ^= 1;
        tamper(
            &path,
            "UPDATE event SET data_ct = ?1 WHERE position = 2",
            Some(bytes),
        );
        let log = open(&path);
        assert_eq!(
            log.read_stream(&stream("task/t1"), 1, 10).unwrap_err(),
            StorageError::Tampered
        );
        assert_eq!(log.read_stream(&stream("task/t1"), 3, 10).unwrap().len(), 1);
    }
}

#[test]
fn verify_chain_points_to_first_altered_or_missing_row() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    assert_eq!(log.verify_chain(), Ok(3));
    drop(log);
    let mut bytes = data_ct(&path, 2);
    bytes[0] ^= 1;
    tamper(
        &path,
        "UPDATE event SET data_ct = ?1 WHERE position = 2",
        Some(bytes),
    );
    assert_eq!(
        open(&path).verify_chain(),
        Err(StorageError::ChainBroken { position: 2 })
    );
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper(&path, "DELETE FROM event WHERE position = 2", None);
    assert_eq!(
        open(&path).verify_chain(),
        Err(StorageError::ChainBroken { position: 3 })
    );
}

#[test]
fn racing_connections_on_same_seq_let_exactly_one_in() {
    for attempt in 0..10u64 {
        let path = fixture();
        let mut first = open(&path);
        first
            .append(&event("task/race", 1, 1, serde_json::json!({})))
            .unwrap();
        let second = open(&path);
        let barrier = std::sync::Barrier::new(2);
        let results: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = [first, second]
                .into_iter()
                .enumerate()
                .map(|(index, mut log)| {
                    let barrier = &barrier;
                    scope.spawn(move || {
                        let e = event(
                            "task/race",
                            2,
                            10 + index as u64,
                            serde_json::json!({ "attempt": attempt }),
                        );
                        barrier.wait();
                        log.append(&e).map(|p| p.get())
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(
            results.iter().filter(|r| **r == Ok(2)).count(),
            1,
            "{results:?}"
        );
        assert_eq!(
            results
                .iter()
                .filter(|r| **r == Err(StorageError::Conflict))
                .count(),
            1,
            "{results:?}"
        );
        let log = open(&path);
        assert_eq!(log.head(&stream("task/race")), Ok(2));
        assert_eq!(log.verify_chain(), Ok(2));
    }
}

#[test]
fn seq_gap_or_repeat_is_refused_by_api_and_database() {
    let path = fixture();
    let mut log = open(&path);
    log.append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    for (name, seq) in [("task/t1", 1), ("task/t1", 3), ("task/new", 2)] {
        assert_eq!(
            log.append(&event(name, seq, 50 + seq, serde_json::json!({})))
                .unwrap_err(),
            StorageError::Conflict
        );
    }
    assert_eq!(log.head(&stream("task/t1")), Ok(1));
    assert_eq!(log.head(&stream("task/new")), Ok(0));
    drop(log);
    let c = Connection::open(&path).unwrap();
    // Copy of the last row, valid in every column except the one each case breaks.
    let insert = |id: &str, seq: &str, position: &str, prev: &str, verb: &str| {
        c.execute(
            &format!(
                "{verb} INTO event SELECT {position}, format, '{id}', type, ts, stream, {seq}, \
                 actor_kind, actor_id, cause, data_class, action_id, approval_id, key_id, \
                 nonce, data_ct, {prev}, chain \
                 FROM event WHERE position = (SELECT max(position) FROM event)"
            ),
            [],
        )
    };
    let constraint = |result: rusqlite::Result<usize>| match result {
        Err(rusqlite::Error::SqliteFailure(e, _)) => {
            assert_eq!(e.code, rusqlite::ErrorCode::ConstraintViolation)
        }
        other => panic!("not refused: {other:?}"),
    };
    let next = "position + 1";
    let new_id = "01J00000000000000000000077";
    constraint(insert(new_id, "seq + 2", next, "chain", "INSERT"));
    constraint(insert(new_id, "seq", next, "chain", "INSERT"));
    constraint(insert(new_id, "seq + 1", "position + 2", "chain", "INSERT"));
    constraint(insert(new_id, "seq + 1", "NULL", "chain", "INSERT"));
    constraint(insert(new_id, "seq + 1", next, "zeroblob(32)", "INSERT"));
    // Replacing an existing row would delete it without the delete trigger.
    constraint(insert(
        "01J00000000000000000000001",
        "seq + 1",
        next,
        "chain",
        "INSERT OR REPLACE",
    ));
    constraint(insert(
        new_id,
        "seq + 1",
        "position",
        "chain",
        "INSERT OR REPLACE",
    ));
    // Control: the same copy with only valid values is accepted.
    assert_eq!(
        insert(new_id, "seq + 1", next, "chain", "INSERT").unwrap(),
        1
    );
}

#[test]
fn update_and_delete_are_refused_by_database() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let c = Connection::open(&path).unwrap();
    for sql in [
        "UPDATE event SET ts = ts",
        "UPDATE event SET data_ct = data_ct WHERE position = 2",
        "DELETE FROM event WHERE position = 3",
        "DELETE FROM event",
        "UPDATE cipher_key SET key_check = key_check",
        "DELETE FROM cipher_key",
        "INSERT OR REPLACE INTO cipher_key SELECT * FROM cipher_key",
    ] {
        match c.execute(sql, []) {
            Err(rusqlite::Error::SqliteFailure(e, _)) => {
                assert_eq!(e.code, rusqlite::ErrorCode::ConstraintViolation, "{sql}")
            }
            other => panic!("not refused: {sql} {other:?}"),
        }
    }
    drop(c);
    let log = open(&path);
    assert_eq!(log.verify_chain(), Ok(3));
    assert_eq!(log.read_stream(&stream("task/t1"), 1, 10).unwrap().len(), 3);
}

#[test]
fn version_one_database_with_data_migrates_keeping_meta() {
    let path = fixture();
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID;
         INSERT INTO meta VALUES ('kept', x'0a0b');",
    )
    .unwrap();
    c.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let c = Connection::open(&path).unwrap();
    let kept: Vec<u8> = c
        .query_row("SELECT value FROM meta WHERE key = 'kept'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(kept, [10, 11]);
    let version: i64 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    drop(c);
    // A version 1 file with an object version 1 never had is not migrated.
    let path = fixture();
    let c = Connection::open(&path).unwrap();
    c.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID;
         CREATE TABLE event (x);",
    )
    .unwrap();
    c.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        AuditStore::open(&path, &StorageConfig::defaults()).unwrap_err(),
        StorageError::UnsupportedSchema
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn reopen_preserves_events_head_and_chain() {
    let path = fixture();
    let mut log = open(&path);
    let events = three(&mut log);
    drop(log);
    let mut log = open(&path);
    assert_eq!(log.head(&stream("task/t1")), Ok(3));
    let fourth = event("task/t1", 4, 4, serde_json::json!({}));
    assert_eq!(log.append(&fourth).unwrap().get(), 4);
    drop(log);
    let log = open(&path);
    let read: Vec<_> = log
        .read_stream(&stream("task/t1"), 2, 10)
        .unwrap()
        .into_iter()
        .map(|s| (s.position.get(), s.event))
        .collect();
    assert_eq!(
        read,
        [(2, events[1].clone()), (3, events[2].clone()), (4, fourth)]
    );
    assert_eq!(log.verify_chain(), Ok(4));
}

#[test]
fn schema_with_removed_or_changed_trigger_is_refused_on_open() {
    for sql in [
        "DROP TRIGGER event_no_update",
        "DROP TRIGGER event_append_order",
        "DROP TRIGGER cipher_key_no_delete",
        "DROP TRIGGER event_no_delete; \
         CREATE TRIGGER event_no_delete BEFORE DELETE ON event BEGIN SELECT 1; END",
        "CREATE INDEX extra ON event(stream)",
    ] {
        let path = fixture();
        let mut log = open(&path);
        three(&mut log);
        drop(log);
        Connection::open(&path).unwrap().execute_batch(sql).unwrap();
        assert_eq!(
            EventLog::open(&path, &StorageConfig::defaults(), keys("key-a", 1)).unwrap_err(),
            StorageError::UnsupportedSchema,
            "{sql}"
        );
    }
}

#[test]
fn oversized_event_is_refused_without_writing_and_reads_are_paged() {
    let path = fixture();
    let config = StorageConfig::from_toml_str("max_event_bytes = 64\nread_page_limit = 2").unwrap();
    let mut log = EventLog::open(&path, &config, keys("key-a", 1)).unwrap();
    let big = event(
        "task/t1",
        1,
        1,
        serde_json::json!({ "text": "x".repeat(64) }),
    );
    assert_eq!(log.append(&big).unwrap_err(), StorageError::TooLarge);
    assert_eq!(log.head(&stream("task/t1")), Ok(0));
    for n in 1..=5 {
        log.append(&event("task/t1", n, n, serde_json::json!({ "n": n })))
            .unwrap();
    }
    let seqs = |from| -> Vec<u64> {
        log.read_stream(&stream("task/t1"), from, 100)
            .unwrap()
            .iter()
            .map(|s| s.event.seq.get())
            .collect()
    };
    assert_eq!(seqs(1), [1, 2]);
    assert_eq!(seqs(3), [3, 4]);
    assert_eq!(seqs(5), [5]);
    assert_eq!(seqs(6), Vec::<u64>::new());
}

fn count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM event", [], |r| r.get(0))
        .unwrap()
}

fn refused(result: rusqlite::Result<usize>, what: &str) {
    match result {
        Err(rusqlite::Error::SqliteFailure(e, _)) => {
            assert_eq!(e.code, rusqlite::ErrorCode::ConstraintViolation, "{what}")
        }
        other => panic!("not refused: {what} {other:?}"),
    }
}

#[test]
fn every_write_verb_that_could_replace_or_skip_is_refused() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let c = Connection::open(&path).unwrap();
    let last_id: String = c
        .query_row("SELECT id FROM event WHERE position = 3", [], |r| r.get(0))
        .unwrap();
    // A copy of the last row with a new id and the next position and link, but
    // the repeated (stream, seq) 3: REPLACE would delete the old row silently.
    let copy = |verb: &str, seq: &str, tail: &str| {
        c.execute(
            &format!(
                "{verb} INTO event SELECT position + 1, format, '01J00000000000000000000077', \
                 type, ts, stream, {seq}, actor_kind, actor_id, cause, data_class, action_id, \
                 approval_id, key_id, nonce, data_ct, chain, chain \
                 FROM event WHERE position = 3 {tail}"
            ),
            [],
        )
    };
    refused(copy("INSERT OR REPLACE", "seq", ""), "insert or replace");
    refused(copy("REPLACE", "seq", ""), "replace into");
    refused(copy("INSERT OR IGNORE", "seq + 2", ""), "insert or ignore");
    refused(
        copy(
            "INSERT",
            "seq",
            "ON CONFLICT (stream, seq) DO UPDATE SET ts = excluded.ts",
        ),
        "upsert update",
    );
    refused(
        copy("INSERT", "seq", "ON CONFLICT DO NOTHING"),
        "upsert nothing",
    );
    refused(
        c.execute("UPDATE OR REPLACE event SET seq = 2 WHERE position = 3", []),
        "update or replace",
    );
    let still: String = c
        .query_row(
            "SELECT id FROM event WHERE stream = 'task/t1' AND seq = 3",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(still, last_id);
    drop(c);
    assert_eq!(count(&path), 3);
    let log = open(&path);
    assert_eq!(log.verify_chain(), Ok(3));
    assert_eq!(log.read_stream(&stream("task/t1"), 1, 10).unwrap().len(), 3);
}

#[test]
fn ciphertext_swapped_between_rows_is_refused() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper(
        &path,
        "UPDATE event SET (nonce, data_ct) = \
         (SELECT nonce, data_ct FROM event WHERE position = 3) WHERE position = 2",
        None,
    );
    let log = open(&path);
    assert_eq!(
        log.read_stream(&stream("task/t1"), 2, 1).unwrap_err(),
        StorageError::Tampered
    );
    assert_eq!(
        log.verify_chain(),
        Err(StorageError::ChainBroken { position: 2 })
    );
}

#[test]
fn reordered_rows_break_the_chain() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper_with(&path, |c| {
        c.execute_batch(
            "UPDATE event SET position = 100 WHERE position = 2;
             UPDATE event SET position = 2 WHERE position = 3;
             UPDATE event SET position = 3 WHERE position = 100;",
        )
        .unwrap();
    });
    assert_eq!(
        open(&path).verify_chain(),
        Err(StorageError::ChainBroken { position: 2 })
    );
}

#[test]
fn cut_tail_passes_but_a_forged_row_does_not() {
    // Known limit: without an anchor outside the file, a cut tail is a valid chain.
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper(&path, "DELETE FROM event WHERE position = 3", None);
    assert_eq!(open(&path).verify_chain(), Ok(2));
    // A plain INSERT, no trigger removed, with every clear column right but a link
    // computed without the key: the database takes it, the chain refuses it.
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let c = Connection::open(&path).unwrap();
    assert_eq!(
        c.execute(
            "INSERT INTO event SELECT 4, format, '01J00000000000000000000004', type, ts, \
             stream, 4, actor_kind, actor_id, NULL, data_class, action_id, approval_id, key_id, \
             randomblob(24), randomblob(40), chain, randomblob(32) \
             FROM event WHERE position = 3",
            [],
        )
        .unwrap(),
        1
    );
    drop(c);
    let log = open(&path);
    assert_eq!(
        log.verify_chain(),
        Err(StorageError::ChainBroken { position: 4 })
    );
    assert_eq!(
        log.read_stream(&stream("task/t1"), 4, 1).unwrap_err(),
        StorageError::Tampered
    );
}

#[test]
fn held_write_lock_gives_busy_without_writing() {
    let path = fixture();
    let config = StorageConfig::from_toml_str("busy_timeout_ms = 0").unwrap();
    // Opened before the lock is taken: opening runs the migration under it.
    let mut log = EventLog::open(&path, &config, keys("key-a", 1)).unwrap();
    let holder = Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let first = event("task/t1", 1, 1, serde_json::json!({}));
    assert_eq!(log.append(&first).unwrap_err(), StorageError::Busy);
    assert_eq!(log.head(&stream("task/t1")), Ok(0));
    holder.execute_batch("COMMIT").unwrap();
    // Busy means retry the same event.
    assert_eq!(log.append(&first).unwrap().get(), 1);
}

#[test]
fn tampered_key_check_is_wrong_key() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper(
        &path,
        "UPDATE cipher_key SET key_check = ?1 WHERE key_id = 'key-a'",
        Some(vec![7; 16]),
    );
    assert_eq!(
        EventLog::open(&path, &StorageConfig::defaults(), keys("key-a", 1)).unwrap_err(),
        StorageError::WrongKey
    );
}

/// Provider whose `current()` can be made to disagree with its own `by_id`.
struct Flaky(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl KeyProvider for Flaky {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        let byte = if self.0.load(Ordering::SeqCst) { 9 } else { 1 };
        Ok((KeyId::new("key-a")?, SecretKey::new(&[byte; 32])))
    }
    fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
        match id.as_str() {
            "key-a" => Ok(SecretKey::new(&[1; 32])),
            _ => Err(CipherError::KeyUnavailable),
        }
    }
}

#[test]
fn provider_whose_current_and_by_id_disagree_is_refused_before_writing() {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let path = fixture();
    assert_eq!(
        EventLog::open(
            &path,
            &StorageConfig::defaults(),
            Box::new(Flaky(flag.clone()))
        )
        .unwrap_err(),
        StorageError::WrongKey
    );
    flag.store(false, Ordering::SeqCst);
    let mut log = EventLog::open(
        &path,
        &StorageConfig::defaults(),
        Box::new(Flaky(flag.clone())),
    )
    .unwrap();
    flag.store(true, Ordering::SeqCst);
    let e = event("task/t1", 1, 1, serde_json::json!({}));
    assert_eq!(log.append(&e).unwrap_err(), StorageError::WrongKey);
    assert_eq!(count(&path), 0);
    flag.store(false, Ordering::SeqCst);
    log.append(&e).unwrap();
    assert_eq!(log.read_stream(&stream("task/t1"), 1, 1).unwrap().len(), 1);
}

fn gate_event(
    kind: &str,
    stream_name: &str,
    seq: u64,
    n: u64,
    data: serde_json::Value,
) -> EventEnvelopeV1 {
    EventEnvelopeV1 {
        r#type: EventEnvelopeV1Type::new(kind).unwrap(),
        ..event(stream_name, seq, n, data)
    }
}

#[test]
fn gate_ids_are_clear_columns_bound_to_the_row() {
    let path = fixture();
    let mut log = open(&path);
    log.append(&gate_event(
        "urn:ads:event:action.planned:1",
        "action/a1",
        1,
        1,
        serde_json::json!({ "action_id": "a1" }),
    ))
    .unwrap();
    log.append(&gate_event(
        "urn:ads:event:approval.requested:2",
        "approval/ap1",
        1,
        2,
        serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
    ))
    .unwrap();
    log.append(&gate_event(
        "urn:ads:event:action.started:1",
        "action/a1",
        2,
        3,
        serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
    ))
    .unwrap();
    log.append(&event(
        "task/t1",
        1,
        4,
        serde_json::json!({ "action_id": "a1" }),
    ))
    .unwrap();
    // The id named by data must be the stream's own.
    assert_eq!(
        log.append(&gate_event(
            "urn:ads:event:action.executed:1",
            "action/a1",
            3,
            5,
            serde_json::json!({ "action_id": "a2", "approval_id": "ap1" }),
        ))
        .unwrap_err(),
        StorageError::InvalidEvent
    );
    drop(log);
    let c = Connection::open(&path).unwrap();
    let ids: Vec<(Option<String>, Option<String>)> = c
        .prepare("SELECT action_id, approval_id FROM event ORDER BY position")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let some = |s: &str| Some(s.to_owned());
    assert_eq!(
        ids,
        [
            (some("a1"), None),
            (some("a1"), some("ap1")),
            (some("a1"), some("ap1")),
            (None, None),
        ]
    );
    // The database refuses an action row whose clear id is not its stream's.
    refused(
        c.execute(
            "INSERT INTO event SELECT 5, format, '01J00000000000000000000005', type, ts, \
             stream, 3, actor_kind, actor_id, cause, data_class, 'a2', approval_id, key_id, \
             nonce, data_ct, (SELECT chain FROM event WHERE position = 4), chain \
             FROM event WHERE position = 3",
            [],
        ),
        "action_id of another stream",
    );
    drop(c);
    for (sql, read) in [
        (
            "UPDATE event SET approval_id = NULL WHERE position = 3",
            "action/a1",
        ),
        (
            "UPDATE event SET action_id = 'a9' WHERE position = 2",
            "approval/ap1",
        ),
    ] {
        let copy = fixture();
        std::fs::copy(&path, &copy).unwrap();
        tamper(&copy, sql, None);
        let log = open(&copy);
        assert_eq!(
            log.read_stream(&stream(read), 1, 10).unwrap_err(),
            StorageError::Tampered,
            "{sql}"
        );
        assert!(
            matches!(log.verify_chain(), Err(StorageError::ChainBroken { .. })),
            "{sql}"
        );
    }
}

#[test]
fn type_and_stream_family_are_bounded() {
    let path = fixture();
    let mut log = open(&path);
    let kind = |n: usize| format!("urn:ads:event:task.{}:1", "a".repeat(n));
    // "urn:ads:event:task." is 19 bytes and ":1" two: 107 letters reach 128.
    let at_limit = gate_event(&kind(107), "task/t1", 1, 1, serde_json::json!({}));
    assert_eq!(at_limit.r#type.as_str().len(), 128);
    log.append(&at_limit).unwrap();
    assert_eq!(
        log.append(&gate_event(
            &kind(108),
            "task/t1",
            2,
            2,
            serde_json::json!({})
        ))
        .unwrap_err(),
        StorageError::InvalidEvent
    );
    let family = |n: usize| format!("{}/x", "f".repeat(n));
    log.append(&event(&family(32), 1, 3, serde_json::json!({})))
        .unwrap();
    assert_eq!(
        log.append(&event(&family(33), 1, 4, serde_json::json!({})))
            .unwrap_err(),
        StorageError::InvalidEvent
    );
    assert_eq!(log.verify_chain(), Ok(2));
}

#[test]
fn event_size_limit_is_inclusive_and_empty_reads_are_empty() {
    let path = fixture();
    let config = StorageConfig::from_toml_str("max_event_bytes = 64").unwrap();
    let mut log = EventLog::open(&path, &config, keys("key-a", 1)).unwrap();
    // The JSON around the text, {"t":""}, takes 8 bytes.
    let sized = |n: usize, seq| {
        event(
            "task/t1",
            seq,
            seq,
            serde_json::json!({ "t": "x".repeat(n) }),
        )
    };
    log.append(&sized(56, 1)).unwrap();
    assert_eq!(
        log.append(&sized(57, 2)).unwrap_err(),
        StorageError::TooLarge
    );
    assert_eq!(log.head(&stream("task/t1")), Ok(1));
    assert!(
        log.read_stream(&stream("task/t1"), 1, 0)
            .unwrap()
            .is_empty()
    );
    assert!(
        log.read_stream(&stream("task/none"), 1, 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        log.read_stream(&stream("task/t1"), 2, 10)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn page_stops_at_the_byte_budget_and_always_makes_progress() {
    let path = fixture();
    let config =
        StorageConfig::from_toml_str("max_event_bytes = 64\nread_page_max_bytes = 150").unwrap();
    let mut log = EventLog::open(&path, &config, keys("key-a", 1)).unwrap();
    // 50 bytes of JSON each, 66 of ciphertext: two fit in 150, three do not.
    for n in 1..=5 {
        log.append(&event(
            "task/t1",
            n,
            n,
            serde_json::json!({ "t": "y".repeat(42) }),
        ))
        .unwrap();
    }
    let seqs = |log: &EventLog, from| -> Vec<u64> {
        log.read_stream(&stream("task/t1"), from, 100)
            .unwrap()
            .iter()
            .map(|s| s.event.seq.get())
            .collect()
    };
    assert_eq!(seqs(&log, 1), [1, 2]);
    assert_eq!(seqs(&log, 3), [3, 4]);
    assert_eq!(seqs(&log, 5), [5]);
    drop(log);
    // A budget smaller than one row still returns that row alone.
    let tight =
        StorageConfig::from_toml_str("max_event_bytes = 64\nread_page_max_bytes = 64").unwrap();
    let log = EventLog::open(&path, &tight, keys("key-a", 1)).unwrap();
    assert_eq!(seqs(&log, 1), [1]);
    assert_eq!(seqs(&log, 2), [2]);
}

/// Version 1 file: `meta` only, marked by the product.
fn version_one(path: &Path) {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute_batch(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID",
    )
    .unwrap();
    c.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
}

#[test]
fn concurrent_openers_of_a_version_one_file_both_succeed() {
    for _ in 0..5 {
        let path = fixture();
        version_one(&path);
        let barrier = std::sync::Barrier::new(2);
        let results: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let (barrier, path) = (&barrier, &path);
                    scope.spawn(move || {
                        barrier.wait();
                        AuditStore::open(path, &StorageConfig::defaults()).map(drop)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(results, [Ok(()), Ok(())]);
        let mut log = open(&path);
        three(&mut log);
        assert_eq!(log.verify_chain(), Ok(3));
    }
}

/// The pre-release version 2 event table, kept only to prove such files are refused.
const RETIRED_EVENT_TABLE: &str = "CREATE TABLE event (
  position INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, type TEXT NOT NULL,
  ts TEXT NOT NULL, stream TEXT NOT NULL, seq INTEGER NOT NULL, actor_kind TEXT NOT NULL,
  actor_id TEXT NOT NULL, cause TEXT, data_class TEXT NOT NULL, key_id TEXT NOT NULL,
  nonce BLOB NOT NULL, data_ct BLOB NOT NULL, prev_chain BLOB NOT NULL, chain BLOB NOT NULL,
  UNIQUE (stream, seq)) STRICT";

#[test]
fn retired_version_two_and_meta_only_current_files_are_refused_unchanged() {
    for (extra, version, expected) in [
        (RETIRED_EVENT_TABLE, 2, StorageError::RetiredSchema),
        ("", 2, StorageError::RetiredSchema),
        ("", SCHEMA_VERSION, StorageError::UnsupportedSchema),
    ] {
        let path = fixture();
        version_one(&path);
        let c = Connection::open(&path).unwrap();
        if !extra.is_empty() {
            c.execute_batch(extra).unwrap();
        }
        c.pragma_update(None, "user_version", version).unwrap();
        c.pragma_update(None, "wal_checkpoint", "TRUNCATE").unwrap();
        drop(c);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            EventLog::open(&path, &StorageConfig::defaults(), keys("key-a", 1)).unwrap_err(),
            expected,
            "{version} {}",
            extra.len()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
