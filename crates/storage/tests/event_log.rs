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

/// Runs `sql` on a raw connection with the append-only triggers dropped, then
/// recreates them with identical text so the schema check still passes.
fn tamper(path: &Path, sql: &str, blob: Option<Vec<u8>>) {
    let c = Connection::open(path).unwrap();
    let triggers: Vec<(String, String)> = c
        .prepare(
            "SELECT name, sql FROM sqlite_schema WHERE type = 'trigger' AND tbl_name = 'event'",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(triggers.len(), 3);
    for (name, _) in &triggers {
        c.execute_batch(&format!("DROP TRIGGER {name}")).unwrap();
    }
    match blob {
        Some(bytes) => assert_eq!(c.execute(sql, [bytes]).unwrap(), 1),
        None => assert_eq!(c.execute(sql, []).unwrap(), 1),
    };
    for (_, sql) in &triggers {
        c.execute_batch(sql).unwrap();
    }
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
                "{verb} INTO event SELECT {position}, '{id}', type, ts, stream, {seq}, \
                 actor_kind, actor_id, cause, data_class, key_id, nonce, data_ct, {prev}, chain \
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
