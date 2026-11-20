use crate::{
    AuditStore, EventLog, StorageConfig, StorageError,
    cipher::{CipherError, KeyId, KeyProvider, SecretKey},
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use ads_contracts::{
    CommonActorV1, CommonActorV1Kind, CommonDataClassV1, CommonIdV1, CommonTimestampV1,
    CommonUlidV1, EventEnvelopeV1, EventEnvelopeV1Stream, EventEnvelopeV1Type,
};
use rusqlite::Connection;
use std::{
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const SENTINEL: &str = "private-sentinel-5f2e9c1d";
const CLEAR_MARKER: &str = "clear-actor-marker-81b3";

#[derive(Clone, Default)]
pub(crate) struct MemoryAnchor(std::sync::Arc<std::sync::Mutex<MemoryAnchorState>>);

#[derive(Default)]
struct MemoryAnchorState {
    commitment: Option<crate::anchor::Commitment>,
    deny_reads: bool,
    deny_writes: bool,
}

impl crate::anchor::AnchorStore for MemoryAnchor {
    fn read(&self) -> Result<Option<crate::anchor::Commitment>, StorageError> {
        let state = self.0.lock().unwrap();
        if state.deny_reads {
            return Err(StorageError::AnchorUnavailable);
        }
        Ok(state.commitment)
    }

    fn store(&self, commitment: &crate::anchor::Commitment) -> Result<(), StorageError> {
        let mut state = self.0.lock().unwrap();
        if state.deny_writes {
            return Err(StorageError::AnchorUnavailable);
        }
        state.commitment = Some(*commitment);
        Ok(())
    }
}

fn anchored(path: &Path, anchor: &MemoryAnchor) -> Result<EventLog, StorageError> {
    EventLog::open(
        path,
        &StorageConfig::defaults(),
        keys("key-a", 1),
        Box::new(anchor.clone()),
    )
}

#[test]
fn external_commitment_refuses_a_cut_tail_and_an_older_copy() {
    for restore_copy in [false, true] {
        let path = fixture();
        let anchor = MemoryAnchor::default();
        let first = event("task/t1", 1, 1, serde_json::json!({}));
        let second = event("task/t1", 2, 2, serde_json::json!({}));
        let mut log = anchored(&path, &anchor).unwrap();
        log.append(&first).unwrap();
        drop(log);
        let old_file = std::fs::read(&path).unwrap();
        let mut log = anchored(&path, &anchor).unwrap();
        log.append(&second).unwrap();
        drop(log);
        if restore_copy {
            std::fs::write(&path, old_file).unwrap();
        } else {
            tamper(&path, "DELETE FROM event WHERE position = 2", None);
        }
        assert_eq!(open(&path).verify_chain(), Ok(1));
        assert_eq!(
            anchored(&path, &anchor).unwrap_err(),
            StorageError::AnchorMismatch
        );
    }
}

#[test]
fn external_commitment_refuses_a_registered_future_key_before_rotation() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let mut log = anchored(&path, &anchor).unwrap();
    log.append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO cipher_key VALUES (?1, 'xchacha20poly1305', zeroblob(24), zeroblob(16))",
            ["key-b"],
        )
        .unwrap();
    assert_eq!(
        log.append(&event("task/t1", 2, 2, serde_json::json!({}))),
        Err(StorageError::AnchorMismatch)
    );
    let error = EventLog::open(
        &path,
        &StorageConfig::defaults(),
        keys("key-b", 2),
        Box::new(anchor),
    )
    .unwrap_err();
    assert_eq!(error, StorageError::AnchorMismatch);
}

#[test]
fn unavailable_anchor_blocks_open_and_rolls_back_append() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    anchor.0.lock().unwrap().deny_reads = true;
    assert_eq!(
        anchored(&path, &anchor).unwrap_err(),
        StorageError::AnchorUnavailable
    );
    assert!(!path.exists());
    anchor.0.lock().unwrap().deny_reads = false;
    let mut log = anchored(&path, &anchor).unwrap();
    let event = event("task/t1", 1, 1, serde_json::json!({}));
    for deny_reads in [true, false] {
        {
            let mut state = anchor.0.lock().unwrap();
            state.deny_reads = deny_reads;
            state.deny_writes = !deny_reads;
        }
        assert_eq!(log.append(&event), Err(StorageError::AnchorUnavailable));
        assert_eq!(log.head(&event.stream), Ok(0));
    }
    anchor.0.lock().unwrap().deny_writes = false;
    assert_eq!(log.append(&event).unwrap().get(), 1);
    assert_eq!(anchored(&path, &anchor).unwrap().head(&event.stream), Ok(1));
}

#[test]
fn recovery_rechecks_the_confirmed_plan_and_never_silently_enrolls_history() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let config = StorageConfig::defaults();
    let key = || keys("key-a", 1);
    let plan = || EventLog::plan_reanchor(&path, &config, key(), &anchor).unwrap();
    let mut log = open(&path);
    log.append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    assert_eq!(
        anchored(&path, &anchor).unwrap_err(),
        StorageError::AnchorMissing
    );
    let stale = plan();
    assert_eq!(stale.summary().previous_position, None);
    assert_eq!(stale.summary().proposed_position, 1);
    log.append(&event("task/t1", 2, 2, serde_json::json!({})))
        .unwrap();
    assert_eq!(
        EventLog::reanchor(&path, &config, key(), &anchor, stale),
        Err(StorageError::AnchorMismatch)
    );
    EventLog::reanchor(&path, &config, key(), &anchor, plan()).unwrap();
    assert_eq!(anchored(&path, &anchor).unwrap().verify_chain(), Ok(2));
    let missing = fixture();
    assert_eq!(
        anchored(&missing, &anchor).unwrap_err(),
        StorageError::AnchorMismatch
    );
    assert!(!missing.exists());
}

#[test]
fn append_refuses_changed_history_on_an_open_connection() {
    for (sql, position) in [
        ("DELETE FROM event WHERE position = 2", 3),
        (
            "UPDATE event SET data_ct = zeroblob(length(data_ct)) WHERE position = 2",
            2,
        ),
    ] {
        let path = fixture();
        let anchor = MemoryAnchor::default();
        let mut log = anchored(&path, &anchor).unwrap();
        for seq in 1..=3 {
            log.append(&event("task/t1", seq, seq, serde_json::json!({})))
                .unwrap();
        }
        drop(log);
        let mut log = anchored(&path, &anchor).unwrap();
        let before = anchor.0.lock().unwrap().commitment;
        tamper(&path, sql, None);

        let next = event("task/t1", 4, 4, serde_json::json!({}));
        for _ in 0..2 {
            assert_eq!(
                log.append(&next),
                Err(StorageError::HistoryChanged { position })
            );
            assert_eq!(anchor.0.lock().unwrap().commitment, before);
            assert_eq!(log.head(&next.stream), Ok(3));
        }
    }
}

#[test]
fn append_accepts_authenticated_history_from_another_connection() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let mut first = anchored(&path, &anchor).unwrap();
    let mut second = anchored(&path, &anchor).unwrap();
    first
        .append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    let next = event("task/t1", 2, 2, serde_json::json!({}));
    assert_eq!(second.append(&next).unwrap().get(), 2);
    assert_eq!(anchored(&path, &anchor).unwrap().verify_chain(), Ok(2));
}

fn anchored_with(path: &Path, anchor: &MemoryAnchor, config: &str) -> EventLog {
    EventLog::open(
        path,
        &StorageConfig::from_toml_str(config).unwrap(),
        keys("key-a", 1),
        Box::new(anchor.clone()),
    )
    .unwrap()
}

/// Flips one byte of a stored blob straight in the file, outside SQLite.
fn flip_in_file(path: &Path, blob: &[u8]) {
    use std::io::{Seek, SeekFrom, Write};
    let bytes = std::fs::read(path).unwrap();
    let starts: Vec<usize> = bytes
        .windows(blob.len())
        .enumerate()
        .filter(|(_, window)| *window == blob)
        .map(|(start, _)| start)
        .collect();
    assert_eq!(starts.len(), 1);
    let at = starts[0] + blob.len() / 2;
    let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(u64::try_from(at).unwrap()))
        .unwrap();
    file.write_all(&[bytes[at] ^ 0x01]).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn a_direct_file_write_is_refused_on_append_only_when_always() {
    for always in [false, true] {
        let path = fixture();
        let anchor = MemoryAnchor::default();
        let config = if always {
            "verify_on_append = 'always'"
        } else {
            ""
        };
        let mut log = anchored_with(&path, &anchor, config);
        // One row per page, so the next append does not rewrite row 2's page.
        let filler = "x".repeat(2500);
        for seq in 1..=3 {
            log.append(&event(
                "task/t1",
                seq,
                seq,
                serde_json::json!({ "f": filler }),
            ))
            .unwrap();
        }
        drop(log);
        let wal = path.with_extension("db-wal");
        assert!(std::fs::metadata(&wal).map_or(true, |m| m.len() == 0));
        let blob = data_ct(&path, 2);
        // Opening verifies the chain, which also loads its pages into the cache.
        let mut log = anchored_with(&path, &anchor, config);
        let before = anchor.0.lock().unwrap().commitment;
        flip_in_file(&path, &blob);

        let next = event("task/t1", 4, 4, serde_json::json!({}));
        if always {
            for _ in 0..2 {
                assert_eq!(
                    log.append(&next),
                    Err(StorageError::HistoryChanged { position: 2 })
                );
                assert_eq!(anchor.0.lock().unwrap().commitment, before);
                assert_eq!(log.head(&next.stream), Ok(3));
            }
        } else {
            // The documented limit: the default mode lets this append through,
            // and the next anchored open is what refuses the history.
            assert_eq!(log.append(&next).unwrap().get(), 4);
            drop(log);
            assert_eq!(
                EventLog::open(
                    &path,
                    &StorageConfig::defaults(),
                    keys("key-a", 1),
                    Box::new(anchor.clone()),
                )
                .unwrap_err(),
                StorageError::ChainBroken { position: 2 }
            );
        }
    }
}

#[test]
fn append_refuses_a_schema_changed_on_an_open_connection() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let mut log = anchored(&path, &anchor).unwrap();
    log.append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    let before = anchor.0.lock().unwrap().commitment;
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TRIGGER event_no_update")
        .unwrap();
    let next = event("task/t1", 2, 2, serde_json::json!({}));
    assert_eq!(log.append(&next), Err(StorageError::UnsupportedSchema));
    assert_eq!(anchor.0.lock().unwrap().commitment, before);
    assert_eq!(log.head(&next.stream), Ok(1));
}

#[test]
fn append_reports_a_key_rotated_elsewhere_as_unavailable_not_as_tampering() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let mut first = anchored(&path, &anchor).unwrap();
    first
        .append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    let mut rotated = EventLog::open(
        &path,
        &StorageConfig::defaults(),
        Box::new(Keys {
            current: ("key-b", 3),
            older: vec![("key-a", 1)],
        }),
        Box::new(anchor.clone()),
    )
    .unwrap();
    rotated
        .append(&event("task/t1", 2, 2, serde_json::json!({})))
        .unwrap();
    let before = anchor.0.lock().unwrap().commitment;
    let next = event("task/t1", 3, 3, serde_json::json!({}));
    assert_eq!(first.append(&next), Err(StorageError::KeyUnavailable));
    assert_eq!(anchor.0.lock().unwrap().commitment, before);
}

#[test]
fn anchored_open_and_recovery_refuse_a_missing_middle_row() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let mut log = anchored(&path, &anchor).unwrap();
    log.append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    log.append(&gate_event(
        "urn:ads:event:action.started:1",
        "action/a1",
        1,
        2,
        serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
    ))
    .unwrap();
    log.append(&event("task/t1", 2, 3, serde_json::json!({})))
        .unwrap();
    drop(log);
    tamper(&path, "DELETE FROM event WHERE position = 2", None);

    let error = StorageError::ChainBroken { position: 3 };
    assert_eq!(anchored(&path, &anchor).unwrap_err(), error);
    assert_eq!(
        EventLog::plan_reanchor(&path, &StorageConfig::defaults(), keys("key-a", 1), &anchor)
            .unwrap_err(),
        error
    );
}

#[test]
fn failed_sqlite_commit_leaves_the_anchor_ahead_until_confirmed_recovery() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let config = StorageConfig::defaults();
    let mut log = anchored(&path, &anchor).unwrap();
    let first = event("task/t1", 1, 1, serde_json::json!({}));
    let second = event("task/t1", 2, 2, serde_json::json!({}));
    log.append(&first).unwrap();

    log.set_commit_hook(Some(|| true));
    assert_eq!(log.append(&second), Err(StorageError::Constraint));
    log.set_commit_hook(None);
    assert_eq!(log.head(&first.stream), Ok(1));
    assert_eq!(log.append(&second), Err(StorageError::AnchorMismatch));
    assert_eq!(
        anchored(&path, &anchor).unwrap_err(),
        StorageError::AnchorMismatch
    );

    let plan = EventLog::plan_reanchor(&path, &config, keys("key-a", 1), &anchor).unwrap();
    assert_eq!(
        plan.summary(),
        crate::anchor::ReanchorSummary {
            previous_position: Some(2),
            proposed_position: 1,
            position_delta: Some(-1),
            same_genesis: Some(true),
            same_registered_keys: Some(true),
        }
    );
    EventLog::reanchor(&path, &config, keys("key-a", 1), &anchor, plan).unwrap();
    assert_eq!(
        anchored(&path, &anchor)
            .unwrap()
            .append(&second)
            .unwrap()
            .get(),
        2
    );
}

#[test]
fn recovery_refuses_a_registered_key_without_an_event() {
    let path = fixture();
    let anchor = MemoryAnchor::default();
    let mut log = anchored(&path, &anchor).unwrap();
    log.append(&event("task/t1", 1, 1, serde_json::json!({})))
        .unwrap();
    drop(log);
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO cipher_key VALUES (?1, 'xchacha20poly1305', zeroblob(24), zeroblob(16))",
            ["key-b"],
        )
        .unwrap();
    assert_eq!(
        EventLog::plan_reanchor(&path, &StorageConfig::defaults(), keys("key-a", 1), &anchor),
        Err(StorageError::AnchorMismatch)
    );
}

fn fixture() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = crate::open::tests::test_directory();
    assert!(root.is_absolute());
    std::fs::create_dir_all(&root).unwrap();
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
pub(crate) struct Keys {
    current: (&'static str, u8),
    older: Vec<(&'static str, u8)>,
}

pub(crate) fn keys(id: &'static str, byte: u8) -> Box<Keys> {
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
    EventLog::open_unanchored(path, &StorageConfig::defaults(), keys("key-a", 1)).unwrap()
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
        EventLog::open_unanchored(&path, &StorageConfig::defaults(), keys("key-a", 2)).unwrap_err(),
        StorageError::WrongKey
    );
    // A new current key cannot read rows written under a different key-a.
    let rotated = Box::new(Keys {
        current: ("key-b", 3),
        older: vec![("key-a", 2)],
    });
    let log = EventLog::open_unanchored(&path, &StorageConfig::defaults(), rotated).unwrap();
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
    let mut log = EventLog::open_unanchored(&path, &StorageConfig::defaults(), rotated).unwrap();
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
    assert_eq!(triggers.len(), 9);
    for (name, _) in &triggers {
        c.execute_batch(&format!("DROP TRIGGER {name}")).unwrap();
    }
    change(&c);
    for (_, sql) in &triggers {
        c.execute_batch(sql).unwrap();
    }
}

/// `tamper_with` for one statement that must change exactly one row.
pub(crate) fn tamper(path: &Path, sql: &str, blob: Option<Vec<u8>>) {
    tamper_with(path, |c| {
        match blob {
            Some(bytes) => assert_eq!(c.execute(sql, [bytes]).unwrap(), 1),
            None => assert_eq!(c.execute(sql, []).unwrap(), 1),
        };
    });
}

pub(crate) fn data_ct(path: &Path, position: i64) -> Vec<u8> {
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
        let mut log =
            EventLog::open_unanchored(&path, &StorageConfig::defaults(), rotated).unwrap();
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
        let log = EventLog::open_unanchored(&path, &StorageConfig::defaults(), rotated).unwrap();
        assert_eq!(
            log.read_stream(&stream(read), 1, 10).unwrap_err(),
            if *column == "key_id" {
                StorageError::ChainBroken { position: 2 }
            } else {
                StorageError::Tampered
            },
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
        let anchor = MemoryAnchor::default();
        let mut first = anchored(&path, &anchor).unwrap();
        first
            .append(&event("task/race", 1, 1, serde_json::json!({})))
            .unwrap();
        let second = anchored(&path, &anchor).unwrap();
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
        let log = anchored(&path, &anchor).unwrap();
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
            EventLog::open_unanchored(&path, &StorageConfig::defaults(), keys("key-a", 1))
                .unwrap_err(),
            StorageError::UnsupportedSchema,
            "{sql}"
        );
    }
}

#[test]
fn oversized_event_is_refused_without_writing_and_reads_are_paged() {
    let path = fixture();
    let config = StorageConfig::from_toml_str("max_event_bytes = 64\nread_page_limit = 2").unwrap();
    let mut log = EventLog::open_unanchored(&path, &config, keys("key-a", 1)).unwrap();
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
    let mut log = EventLog::open_unanchored(&path, &config, keys("key-a", 1)).unwrap();
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
        EventLog::open_unanchored(&path, &StorageConfig::defaults(), keys("key-a", 1)).unwrap_err(),
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
        EventLog::open_unanchored(
            &path,
            &StorageConfig::defaults(),
            Box::new(Flaky(flag.clone()))
        )
        .unwrap_err(),
        StorageError::WrongKey
    );
    flag.store(false, Ordering::SeqCst);
    let mut log = EventLog::open_unanchored(
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
    let mut log = EventLog::open_unanchored(&path, &config, keys("key-a", 1)).unwrap();
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
    let mut log = EventLog::open_unanchored(&path, &config, keys("key-a", 1)).unwrap();
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
    let log = EventLog::open_unanchored(&path, &tight, keys("key-a", 1)).unwrap();
    assert_eq!(seqs(&log, 1), [1]);
    assert_eq!(seqs(&log, 2), [2]);
}

#[test]
fn reading_numeric_arrays_measures_retained_event_memory() {
    use serde_json::Value;

    let path = fixture();
    let config =
        StorageConfig::from_toml_str("max_event_bytes = 20000\nread_page_max_bytes = 40000")
            .unwrap();
    let mut log = EventLog::open_unanchored(&path, &config, keys("key-a", 1)).unwrap();
    for n in 1..=3 {
        log.append(&event(
            "task/t1",
            n,
            n,
            serde_json::json!({ "numbers": vec![0; 8192] }),
        ))
        .unwrap();
    }
    let page = log.read_stream(&stream("task/t1"), 1, 10).unwrap();
    assert_eq!(page.len(), 2);
    let row_bytes: u32 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT length(data_ct) FROM event WHERE position = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let page_slots = page.capacity() * std::mem::size_of::<crate::StoredEvent>();
    let mut array_bytes = 0;
    for event in &page {
        let Value::Array(numbers) = &event.event.data["numbers"] else {
            panic!("expected numeric array");
        };
        assert_eq!(numbers.len(), 8192);
        array_bytes += numbers.capacity() * std::mem::size_of::<Value>();
    }
    eprintln!(
        "ciphertext per event: {row_bytes} B; retained per event: {} B of array slots; page: {} B of array and event slots",
        array_bytes / page.len(),
        array_bytes + page_slots,
    );
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
            EventLog::open_unanchored(&path, &StorageConfig::defaults(), keys("key-a", 1))
                .unwrap_err(),
            expected,
            "{version} {}",
            extra.len()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        if version == 2 {
            assert_eq!(
                std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
                1
            );
        }
    }
}

#[test]
fn refused_identity_in_pending_wal_never_reaches_main_file() {
    for (pragma, value, error) in [
        ("user_version", 2, StorageError::RetiredSchema),
        (
            "user_version",
            SCHEMA_VERSION + 1,
            StorageError::SchemaTooNew,
        ),
        (
            "application_id",
            APPLICATION_ID + 1,
            StorageError::ForeignApplication,
        ),
    ] {
        assert_pending_wal_refusal(&[(pragma, value)], error);
    }
}

#[test]
fn product_wal_cannot_clear_the_database_identity() {
    assert_pending_wal_refusal(
        &[("application_id", 0), ("user_version", 0)],
        StorageError::ForeignApplication,
    );
}

fn assert_pending_wal_refusal(pragmas: &[(&str, i64)], error: StorageError) {
    let source = fixture();
    version_one(&source);
    let mut writer = Connection::open(&source).unwrap();
    writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    let tx = writer.transaction().unwrap();
    for &(pragma, value) in pragmas {
        tx.pragma_update(None, pragma, value).unwrap();
    }
    tx.commit().unwrap();
    let path = fixture();
    let wal_path = path.with_file_name("audit.db-wal");
    let main = std::fs::read(&source).unwrap();
    let wal = std::fs::read(source.with_file_name("audit.db-wal")).unwrap();
    assert_eq!(i32::from_be_bytes(main[60..64].try_into().unwrap()), 1);
    std::fs::write(&path, &main).unwrap();
    std::fs::write(&wal_path, &wal).unwrap();
    assert_eq!(
        AuditStore::open(&path, &StorageConfig::defaults()).unwrap_err(),
        error
    );
    assert_eq!(std::fs::read(&path).unwrap(), main);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal);
    assert!(!path.with_file_name("audit.db-shm").exists());
}

#[test]
fn external_connection_cannot_modify_genesis() {
    let path = fixture();
    drop(open(&path));
    let connection = Connection::open(&path).unwrap();
    for sql in [
        "UPDATE log_genesis SET link = zeroblob(32)",
        "DELETE FROM log_genesis",
        "INSERT OR REPLACE INTO log_genesis VALUES (1, zeroblob(32))",
        "INSERT INTO log_genesis VALUES (1, zeroblob(32)) ON CONFLICT(singleton) DO UPDATE SET link = excluded.link",
    ] {
        let error = connection.execute(sql, []).unwrap_err();
        assert!(
            matches!(error, rusqlite::Error::SqliteFailure(_, Some(message)) if message == "append-only")
        );
    }
    connection
        .execute_batch("DROP TRIGGER log_genesis_no_delete")
        .unwrap();
    assert_eq!(
        AuditStore::open(&path, &StorageConfig::defaults()).unwrap_err(),
        StorageError::UnsupportedSchema
    );
}

#[test]
fn missing_genesis_breaks_the_chain_at_position_one() {
    let path = fixture();
    drop(open(&path));
    let connection = Connection::open(&path).unwrap();
    let trigger: Option<String> = connection
        .query_row(
            "SELECT (SELECT sql FROM sqlite_schema WHERE name = 'log_genesis_no_delete')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute_batch("DROP TRIGGER IF EXISTS log_genesis_no_delete; DELETE FROM log_genesis")
        .unwrap();
    if let Some(sql) = trigger {
        connection.execute_batch(&sql).unwrap();
    }
    drop(connection);
    let mut log = open(&path);
    let error = StorageError::ChainBroken { position: 1 };
    assert_eq!(log.verify_chain(), Err(error));
    assert_eq!(
        log.append(&event("task/t1", 1, 1, serde_json::json!({}))),
        Err(error)
    );
}

#[test]
fn genesis_is_distinct_per_database_and_binds_the_first_row() {
    let first_link = |path: &Path| {
        Connection::open(path)
            .unwrap()
            .query_row("SELECT prev_chain FROM event WHERE position = 1", [], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .unwrap()
    };
    let first = fixture();
    let second = fixture();
    for path in [&first, &second] {
        let mut log = open(path);
        three(&mut log);
        assert_eq!(log.verify_chain(), Ok(3));
    }
    assert_ne!(first_link(&first), [0; 32]);
    assert_ne!(first_link(&first), first_link(&second));
    let other = first_link(&second);
    tamper(
        &first,
        "UPDATE event SET prev_chain = ?1 WHERE position = 1",
        Some(other),
    );
    assert_eq!(
        open(&first).verify_chain(),
        Err(StorageError::ChainBroken { position: 1 })
    );
}

#[test]
fn an_authenticated_chain_cannot_return_to_an_older_key() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let config = StorageConfig::defaults();
    let mut log = EventLog::open_unanchored(&path, &config, keys("key-b", 2)).unwrap();
    log.append(&event("task/t1", 4, 4, serde_json::json!({})))
        .unwrap();
    drop(log);
    let mut log = open(&path);
    let before = files(&path);
    assert_eq!(
        log.append(&event("task/t1", 5, 5, serde_json::json!({}))),
        Err(StorageError::WrongKey)
    );
    assert_eq!(files(&path), before);
    let log = EventLog::open_unanchored(
        &path,
        &config,
        Box::new(Keys {
            current: ("key-b", 2),
            older: vec![("key-a", 1)],
        }),
    )
    .unwrap();
    assert_eq!(log.verify_chain(), Ok(4));
}

#[test]
fn read_stream_checks_alternative_registrations_within_budget() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper_with(&path, |c| {
        c.execute_batch(
            "INSERT INTO cipher_key VALUES ('key-b', 'xchacha20poly1305', zeroblob(24), zeroblob(16));
             UPDATE event SET key_id = 'key-b' WHERE position = 2;
             UPDATE cipher_key SET key_check = zeroblob(16) WHERE key_id = 'key-a';",
        ).unwrap();
    });
    for n in 0..12 {
        Connection::open(&path).unwrap().execute(
            "INSERT INTO cipher_key VALUES (?1, 'xchacha20poly1305', zeroblob(24), zeroblob(16))",
            [format!("other-{n:02}")],
        ).unwrap();
    }
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let config = StorageConfig::from_toml_str("read_key_candidates = 2").unwrap();
    // Open under a fresh id so the read, rather than open, checks both registrations.
    struct ReaderKeys(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    impl KeyProvider for ReaderKeys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new("reader")?, SecretKey::new(&[3; 32])))
        }

        fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
            if id.as_str() != "reader" {
                self.0.lock().unwrap().push(id.as_str().to_owned());
            }
            let byte = match id.as_str() {
                "key-a" => 1,
                "key-b" => 2,
                "reader" => 3,
                _ => return Err(CipherError::KeyUnavailable),
            };
            Ok(SecretKey::new(&[byte; 32]))
        }
    }
    let log =
        EventLog::open_unanchored(&path, &config, Box::new(ReaderKeys(calls.clone()))).unwrap();
    assert_eq!(
        log.read_stream(&stream("task/t1"), 2, 1).unwrap_err(),
        StorageError::WrongKey
    );
    assert_eq!(*calls.lock().unwrap(), ["key-b", "key-a", "other-00"]);
}

#[test]
fn read_stream_reports_the_position_of_a_forged_key_id() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    tamper_with(&path, |c| {
        c.pragma_update(None, "foreign_keys", 0).unwrap();
        c.execute("UPDATE event SET key_id = 'forged' WHERE position = 2", [])
            .unwrap();
    });
    assert_eq!(
        open(&path)
            .read_stream(&stream("task/t1"), 1, 10)
            .unwrap_err(),
        StorageError::ChainBroken { position: 2 }
    );
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO cipher_key VALUES ('forged', 'xchacha20poly1305', zeroblob(24), zeroblob(16))",
            [],
        )
        .unwrap();
    let config = StorageConfig::from_toml_str("read_key_candidates = 0").unwrap();
    let log = EventLog::open_unanchored(&path, &config, keys("key-a", 1)).unwrap();
    assert_eq!(
        log.read_stream(&stream("task/t1"), 1, 10).unwrap_err(),
        StorageError::ChainBroken { position: 2 }
    );
    assert_eq!(
        log.verify_chain(),
        Err(StorageError::ChainBroken { position: 2 })
    );
}

/// `action/a1`, `approval/ap1` and `task/t1`, one event each.
fn gate_log(path: &Path) {
    let mut log = open(path);
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
    log.append(&event("task/t1", 1, 3, serde_json::json!({})))
        .unwrap();
}

/// Plain INSERT, triggers intact: a copy of position 1 continuing the chain
/// and the stream, with the given stream, type and clear ids.
fn insert_copy(
    c: &Connection,
    stream_name: &str,
    kind: &str,
    action_id: Option<&str>,
    approval_id: Option<&str>,
) -> rusqlite::Result<usize> {
    c.execute(
        "INSERT INTO event SELECT (SELECT max(position) FROM event) + 1, format, \
         printf('01J%023d', (SELECT max(position) FROM event) + 100), ?2, ts, ?1, \
         coalesce((SELECT max(seq) FROM event WHERE stream = ?1), 0) + 1, actor_kind, \
         actor_id, cause, data_class, ?3, ?4, key_id, nonce, data_ct, \
         (SELECT chain FROM event ORDER BY position DESC LIMIT 1), chain \
         FROM event WHERE position = 1",
        rusqlite::params![stream_name, kind, action_id, approval_id],
    )
}

#[test]
fn database_binds_gate_ids_and_type_family_to_the_stream_family() {
    let path = fixture();
    gate_log(&path);
    let c = Connection::open(&path).unwrap();
    let planned = "urn:ads:event:action.planned:1";
    let granted = "urn:ads:event:approval.granted:2";
    let noted = "urn:ads:event:task.noted:1";
    for (stream_name, kind, action_id, approval_id, what) in [
        (
            "approval/ap1",
            granted,
            Some("a1"),
            None,
            "approval id missing",
        ),
        (
            "approval/ap1",
            granted,
            Some("a1"),
            Some("ap2"),
            "approval id of another stream",
        ),
        ("action/a1", planned, None, None, "action id missing"),
        (
            "task/x",
            noted,
            None,
            Some("ap1"),
            "approval id outside the gate",
        ),
        (
            "task/x",
            noted,
            Some("a1"),
            None,
            "action id outside the gate",
        ),
        (
            "task/x",
            "urn:ads:event:action.started:1",
            None,
            None,
            "action type on task",
        ),
        ("task/x", granted, None, None, "approval type on task"),
        ("action/a1", noted, Some("a1"), None, "task type on action"),
        (
            "approval/ap1",
            noted,
            None,
            Some("ap1"),
            "task type on approval",
        ),
        (
            "action/a1",
            granted,
            Some("a1"),
            None,
            "approval type on action",
        ),
        (
            "approval/ap1",
            planned,
            Some("a1"),
            Some("ap1"),
            "action type on approval",
        ),
    ] {
        refused(
            insert_copy(&c, stream_name, kind, action_id, approval_id),
            what,
        );
    }
    // Controls: the same copy with consistent columns is taken by the database.
    for (stream_name, kind, action_id, approval_id) in [
        ("task/x", noted, None, None),
        ("approval/ap1", granted, Some("a1"), Some("ap1")),
        ("action/a1", planned, Some("a1"), None),
    ] {
        assert_eq!(
            insert_copy(&c, stream_name, kind, action_id, approval_id).unwrap(),
            1,
            "{stream_name}"
        );
    }
}

#[test]
fn verify_chain_reports_the_position_of_rows_it_cannot_link() {
    // A row under a key id registered by the forger: the provider has no key.
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let c = Connection::open(&path).unwrap();
    c.execute(
        "INSERT INTO cipher_key VALUES ('zz', 'xchacha20poly1305', zeroblob(24), zeroblob(16))",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO event SELECT 4, format, '01J00000000000000000000004', type, ts, stream, \
         4, actor_kind, actor_id, NULL, data_class, action_id, approval_id, 'zz', nonce, \
         data_ct, chain, randomblob(32) FROM event WHERE position = 3",
        [],
    )
    .unwrap();
    drop(c);
    assert_eq!(
        open(&path).verify_chain(),
        Err(StorageError::ChainBroken { position: 4 })
    );
    // A row of a format this build does not know.
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let c = Connection::open(&path).unwrap();
    c.execute(
        "INSERT INTO event SELECT 4, 2, '01J00000000000000000000004', type, ts, stream, 4, \
         actor_kind, actor_id, NULL, data_class, action_id, approval_id, key_id, nonce, \
         data_ct, chain, chain FROM event WHERE position = 3",
        [],
    )
    .unwrap();
    drop(c);
    assert_eq!(
        open(&path).verify_chain(),
        Err(StorageError::ChainBroken { position: 4 })
    );
}

#[test]
fn altered_format_and_lost_old_key() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let copy = fixture();
    std::fs::copy(&path, &copy).unwrap();
    tamper(
        &copy,
        "UPDATE event SET format = 2 WHERE position = 2",
        None,
    );
    let log = open(&copy);
    assert_eq!(
        log.read_stream(&stream("task/t1"), 2, 1).unwrap_err(),
        StorageError::UnsupportedSchema
    );
    assert_eq!(
        log.verify_chain(),
        Err(StorageError::ChainBroken { position: 2 })
    );
    // A missing historical key makes row 1 unverifiable from both entry points.
    let lost =
        EventLog::open_unanchored(&path, &StorageConfig::defaults(), keys("key-b", 3)).unwrap();
    assert_eq!(
        lost.read_stream(&stream("task/t1"), 1, 1).unwrap_err(),
        StorageError::ChainBroken { position: 1 }
    );
    assert_eq!(
        lost.verify_chain(),
        Err(StorageError::ChainBroken { position: 1 })
    );
}

#[test]
fn tampering_right_after_a_key_rotation_breaks_the_chain_there() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    drop(log);
    let rotated = || {
        Box::new(Keys {
            current: ("key-b", 3),
            older: vec![("key-a", 1)],
        })
    };
    let mut log = EventLog::open_unanchored(&path, &StorageConfig::defaults(), rotated()).unwrap();
    log.append(&event("task/t1", 4, 4, serde_json::json!({ "n": 4 })))
        .unwrap();
    log.append(&event("task/t1", 5, 5, serde_json::json!({ "n": 5 })))
        .unwrap();
    assert_eq!(log.verify_chain(), Ok(5));
    drop(log);
    tamper(
        &path,
        "UPDATE event SET ts = '2026-11-19T12:00:09Z' WHERE position = 4",
        None,
    );
    let log = EventLog::open_unanchored(&path, &StorageConfig::defaults(), rotated()).unwrap();
    assert_eq!(
        log.verify_chain(),
        Err(StorageError::ChainBroken { position: 4 })
    );
}

/// Provider whose `current()` hands out other key material from call `flip` on,
/// while `by_id` stays the same: the material changes in the middle of a call.
struct Drifting {
    calls: std::sync::Arc<AtomicU64>,
    flip: u64,
}
impl KeyProvider for Drifting {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let byte = if call >= self.flip { 9 } else { 1 };
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
fn provider_changing_during_append_never_leaves_an_unreadable_row() {
    for flip in 0..8 {
        let path = fixture();
        let calls = std::sync::Arc::new(AtomicU64::new(0));
        let provider = Box::new(Drifting {
            calls: calls.clone(),
            flip,
        });
        let Ok(mut log) = EventLog::open_unanchored(&path, &StorageConfig::defaults(), provider)
        else {
            continue;
        };
        let opened = calls.load(Ordering::SeqCst);
        for n in 1..=3 {
            let e = event("task/t1", n, n, serde_json::json!({ "n": n }));
            match log.append(&e) {
                Ok(_) => {}
                Err(error) => {
                    assert_eq!(error, StorageError::WrongKey, "flip {flip}");
                    break;
                }
            }
        }
        // Whatever was written reads back and links, wherever the flip landed.
        let head = log.head(&stream("task/t1")).unwrap();
        assert_eq!(
            log.read_stream(&stream("task/t1"), 1, 10).unwrap().len() as u64,
            head,
            "flip {flip} after {opened}"
        );
        assert_eq!(log.verify_chain(), Ok(head), "flip {flip}");
    }
}

#[test]
fn read_stream_refuses_a_missing_seq_inside_the_page() {
    let path = fixture();
    let mut log = open(&path);
    three(&mut log);
    log.append(&event("task/t1", 4, 4, serde_json::json!({})))
        .unwrap();
    drop(log);
    tamper(&path, "DELETE FROM event WHERE position = 2", None);
    let log = open(&path);
    assert_eq!(
        log.read_stream(&stream("task/t1"), 1, 10).unwrap_err(),
        StorageError::ChainBroken { position: 3 }
    );
    // Starting at the missing seq is refused too.
    assert_eq!(
        log.read_stream(&stream("task/t1"), 2, 10).unwrap_err(),
        StorageError::ChainBroken { position: 3 }
    );
    // After the gap the rows still read.
    assert_eq!(log.read_stream(&stream("task/t1"), 3, 10).unwrap().len(), 2);
}
