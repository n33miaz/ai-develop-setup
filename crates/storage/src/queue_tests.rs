use crate::{
    EventLog, StorageConfig, StorageError,
    event_log_tests::{MemoryAnchor, data_ct, keys, tamper},
    queue::queue_stream,
};
use ads_contracts::{CommonIdV1, EventEnvelopeV1};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// A database folder inside the repository's `.test-tmp`, removed file by file on drop.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        assert!(root.ends_with(".test-tmp"));
        loop {
            let dir = root.join(format!(
                "queue-store-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Self { dir },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("fixture directory: {error}"),
            }
        }
    }

    fn db(&self) -> PathBuf {
        self.dir.join("audit.db")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in [
            "audit.db",
            "audit.db-wal",
            "audit.db-shm",
            "audit.db-journal",
        ] {
            let _ = std::fs::remove_file(self.dir.join(name));
        }
        let _ = std::fs::remove_dir(&self.dir);
    }
}

fn open(path: &Path, anchor: &MemoryAnchor, config: &str) -> Result<EventLog, StorageError> {
    EventLog::open(
        path,
        &StorageConfig::from_toml_str(config).unwrap(),
        keys("key-a", 1),
        Box::new(anchor.clone()),
    )
}

fn queue_id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn envelope(seq: u64, kind: &str, actor: (&str, &str), data: serde_json::Value) -> EventEnvelopeV1 {
    serde_json::from_value(serde_json::json!({
        "id": format!("01ARZ3NDEKTSV4RR{seq:010}"),
        "type": format!("urn:ads:event:queue.{kind}"),
        "ts": format!("2030-01-02T03:04:{seq:02}Z"),
        "stream": "queue/queue-01",
        "seq": seq,
        "actor": { "kind": actor.0, "id": actor.1 },
        "cause": null,
        "data_class": "internal",
        "data": data,
    }))
    .unwrap()
}

fn policy(admission_limit: u64) -> serde_json::Value {
    serde_json::json!({
        "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui",
        "policy": {
            "attempt_limit": 2, "admission_limit": admission_limit, "window_seconds": 3600,
            "window_admissions": 10, "manual_window_admissions": 10,
            "bug_rule_window_admissions": 3, "clock_skew_seconds": 5,
            "clock_rewind_seconds": 300, "host_confirm_seconds": 60
        }
    })
}

const DEVICE: (&str, &str) = ("device", "device-01");

/// Queue created, policy, enabled, one manual task started, then the owner lowers the policy.
fn queue_events() -> Vec<EventEnvelopeV1> {
    vec![
        envelope(
            1,
            "created:1",
            DEVICE,
            serde_json::json!({
                "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui",
                "state": "stopped"
            }),
        ),
        envelope(2, "policy_changed:1", DEVICE, policy(1000)),
        envelope(
            3,
            "enabled:1",
            DEVICE,
            serde_json::json!({ "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui" }),
        ),
        envelope(
            4,
            "task_created:3",
            DEVICE,
            serde_json::json!({
                "queue_id": "queue-01", "task_id": "task-01", "attempt_limit": 2,
                "origin": { "kind": "manual", "request_id": "request-01" },
                "workspace_alias": "docs", "state": "queued", "attempts": 0
            }),
        ),
        envelope(
            5,
            "task_state_changed:2",
            ("daemon", "core-01"),
            serde_json::json!({
                "queue_id": "queue-01", "task_id": "task-01", "from": "queued",
                "to": "running", "attempts": 1
            }),
        ),
        envelope(6, "policy_changed:1", DEVICE, policy(1)),
    ]
}

fn read_back(log: &EventLog) -> Result<Vec<EventEnvelopeV1>, StorageError> {
    Ok(log
        .read_queue(&queue_id("queue-01"))?
        .into_iter()
        .map(|stored| stored.event)
        .collect())
}

#[test]
fn a_queue_is_written_and_read_back_after_reopening() {
    let fixture = Fixture::new();
    let anchor = MemoryAnchor::default();
    let events = queue_events();
    let mut log = open(&fixture.db(), &anchor, "").unwrap();
    // Another queue interleaved, so the read follows the stream's own `seq`.
    for event in &events {
        let mut other = event.clone();
        other.id =
            ads_contracts::CommonUlidV1::new(format!("01ARZ3NDEKTSV4RS{:010}", event.seq)).unwrap();
        other.stream = queue_stream(&queue_id("queue-02"));
        other.data["queue_id"] = "queue-02".into();
        log.append(event).unwrap();
        log.append(&other).unwrap();
    }
    drop(log);

    // Pages of one event cross every page boundary of the stream.
    for config in ["", "read_page_limit = 1"] {
        let log = open(&fixture.db(), &anchor, config).unwrap();
        assert_eq!(read_back(&log).unwrap(), events, "{config}");
        assert_eq!(log.head(&queue_stream(&queue_id("queue-01"))), Ok(6));
        assert_eq!(log.read_queue(&queue_id("queue-02")).unwrap().len(), 6);
        assert!(log.read_queue(&queue_id("queue-03")).unwrap().is_empty());
    }
}

#[test]
fn queue_events_outside_their_queue_stream_are_refused() {
    let fixture = Fixture::new();
    let anchor = MemoryAnchor::default();
    let mut log = open(&fixture.db(), &anchor, "").unwrap();
    let created = queue_events().remove(0);

    let mut other_queue = created.clone();
    other_queue.data["queue_id"] = "queue-02".into();
    let mut no_queue = created.clone();
    no_queue.data.remove("queue_id");
    let mut other_family = created.clone();
    other_family.r#type =
        ads_contracts::EventEnvelopeV1Type::new("urn:ads:event:task.noted:1").unwrap();
    let mut other_stream = created.clone();
    other_stream.stream = ads_contracts::EventEnvelopeV1Stream::new("task/queue-01").unwrap();
    for event in [other_queue, no_queue, other_family, other_stream] {
        assert_eq!(log.append(&event), Err(StorageError::InvalidEvent));
    }
    assert_eq!(log.verify_chain(), Ok(0));
    log.append(&created).unwrap();
}

#[test]
fn a_tampered_queue_event_is_refused() {
    let events = queue_events();
    let cases: &[(&str, &str)] = &[
        (
            "ts",
            "UPDATE event SET ts = '2030-01-02T03:03:00Z' WHERE position = 4",
        ),
        (
            "actor",
            "UPDATE event SET actor_id = 'device-02' WHERE position = 4",
        ),
        ("data", ""),
        ("deleted", "DELETE FROM event WHERE position = 5"),
    ];
    for (what, sql) in cases {
        let fixture = Fixture::new();
        let anchor = MemoryAnchor::default();
        let mut log = open(&fixture.db(), &anchor, "").unwrap();
        for event in &events {
            log.append(event).unwrap();
        }
        drop(log);
        if *what == "data" {
            let mut bytes = data_ct(&fixture.db(), 4);
            bytes[0] ^= 1;
            tamper(
                &fixture.db(),
                "UPDATE event SET data_ct = ?1 WHERE position = 4",
                Some(bytes),
            );
        } else {
            tamper(&fixture.db(), sql, None);
        }
        let expected = if *what == "deleted" {
            StorageError::ChainBroken { position: 6 }
        } else {
            StorageError::ChainBroken { position: 4 }
        };
        assert_eq!(
            open(&fixture.db(), &anchor, "").unwrap_err(),
            expected,
            "{what}"
        );
        let unanchored =
            EventLog::open_unanchored(&fixture.db(), &StorageConfig::defaults(), keys("key-a", 1))
                .unwrap();
        let read = read_back(&unanchored).unwrap_err();
        if *what == "deleted" {
            assert_eq!(read, StorageError::ChainBroken { position: 6 }, "{what}");
        } else {
            assert_eq!(read, StorageError::Tampered, "{what}");
        }
    }
}

#[test]
fn a_lowered_cap_keeps_the_queue_history_readable() {
    let fixture = Fixture::new();
    let anchor = MemoryAnchor::default();
    let events = queue_events();
    let mut log = open(&fixture.db(), &anchor, "").unwrap();
    for event in &events {
        log.append(event).unwrap();
    }
    drop(log);

    let mut log = open(&fixture.db(), &anchor, "queue_max_events = 3").unwrap();
    assert_eq!(read_back(&log).unwrap(), events);
    let next = envelope(
        7,
        "stopped:1",
        ("daemon", "core-01"),
        serde_json::json!({ "queue_id": "queue-01", "reason": "restart" }),
    );
    let enabled = envelope(
        7,
        "enabled:1",
        DEVICE,
        serde_json::json!({ "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui" }),
    );
    assert_eq!(log.append(&enabled), Err(StorageError::QueueFull));
    assert_eq!(log.verify_chain(), Ok(6));
    // A full queue can still be stopped, and nothing else enters after the stop.
    log.append(&next).unwrap();
    let mut after = enabled.clone();
    after.id = ads_contracts::CommonUlidV1::new("01ARZ3NDEKTSV4RR0000000008").unwrap();
    after.seq = std::num::NonZeroU64::new(8).unwrap();
    assert_eq!(log.append(&after), Err(StorageError::QueueFull));
    drop(log);

    let mut log = open(&fixture.db(), &anchor, "queue_max_events = 8").unwrap();
    log.append(&after).unwrap();
    let mut grown = events;
    grown.extend([next, after]);
    assert_eq!(read_back(&log).unwrap(), grown);
}

#[test]
fn over_the_cap_only_one_replayable_stop_enters() {
    let fixture = Fixture::new();
    let anchor = MemoryAnchor::default();
    let events = queue_events();
    let mut log = open(&fixture.db(), &anchor, "queue_max_events = 6").unwrap();
    for event in &events {
        log.append(event).unwrap();
    }
    let stop = |seq: u64, kind: &str| {
        let mut event = envelope(
            seq,
            kind,
            ("daemon", "core-01"),
            serde_json::json!({ "queue_id": "queue-01", "reason": "restart" }),
        );
        event.id = ads_contracts::CommonUlidV1::new(format!("01ARZ3NDEKTSV4RT{seq:010}")).unwrap();
        event
    };
    // A version the replay would refuse stays behind the cap.
    assert_eq!(
        log.append(&stop(7, "stopped:2")),
        Err(StorageError::QueueFull)
    );
    // A `seq` past the head is a conflict before the cap is looked at.
    assert_eq!(
        log.append(&stop(8, "stopped:1")),
        Err(StorageError::Conflict)
    );
    log.append(&stop(7, "stopped:1")).unwrap();
    // A second stop over the cap is refused: the storage bounds them, not the producer.
    assert_eq!(
        log.append(&stop(8, "stopped:1")),
        Err(StorageError::QueueFull)
    );
    assert_eq!(log.verify_chain(), Ok(7));
    assert_eq!(log.head(&queue_stream(&queue_id("queue-01"))), Ok(7));
}

/// Takes a current file back to `version` (5, before the database bound queue events, or 4,
/// also before the gate journal).
fn back_to_version(path: &Path, version: i64) {
    let c = rusqlite::Connection::open(path).unwrap();
    c.execute_batch(crate::migrate::DROP_QUEUE_BINDING_SQL)
        .unwrap();
    if version == 4 {
        c.execute_batch(crate::migrate::DROP_GATE_JOURNAL_SQL)
            .unwrap();
    }
    c.pragma_update(None, "user_version", version).unwrap();
    assert_eq!(
        crate::migrate::schema(&c).unwrap(),
        crate::migrate::expected_schema(version).unwrap()
    );
}

/// Copies the last row as the next one, in `stream` with `kind`; only the database checks it.
fn raw_append(path: &Path, kind: &str, stream: &str, seq: i64) -> Result<usize, String> {
    rusqlite::Connection::open(path)
        .unwrap()
        .execute(
            "INSERT INTO event (position, format, id, type, ts, stream, seq, actor_kind, actor_id,
                cause, data_class, action_id, approval_id, key_id, nonce, data_ct, prev_chain, chain)
             SELECT position + 1, format, '01ARZ3NDEKTSV4RT' || printf('%010d', position + 1),
                ?1, ts, ?2, ?3, actor_kind, actor_id, cause, data_class, NULL, NULL, key_id,
                nonce, data_ct, chain, chain
             FROM event ORDER BY position DESC LIMIT 1",
            rusqlite::params![kind, stream, seq],
        )
        .map_err(|error| error.to_string())
}

fn user_version(path: &Path) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn older_queue_rows_migrate_and_the_database_binds_new_ones() {
    for version in [4, 5] {
        older_queue_rows_migrate_from(version);
    }
}

fn older_queue_rows_migrate_from(version: i64) {
    let fixture = Fixture::new();
    let anchor = MemoryAnchor::default();
    let events = queue_events();
    let mut log = open(&fixture.db(), &anchor, "").unwrap();
    for event in &events[..3] {
        log.append(event).unwrap();
    }
    drop(log);
    back_to_version(&fixture.db(), version);

    let mut log = open(&fixture.db(), &anchor, "").unwrap();
    assert_eq!(user_version(&fixture.db()), crate::open::SCHEMA_VERSION);
    assert_eq!(log.verify_chain(), Ok(3));
    assert_eq!(read_back(&log).unwrap(), events[..3]);
    for event in &events[3..] {
        log.append(event).unwrap();
    }
    assert_eq!(read_back(&log).unwrap(), events);
    drop(log);

    let queue_type = "urn:ads:event:queue.enabled:1";
    // Each row is otherwise valid for the chain trigger, so only the binding refuses it.
    for (kind, stream, seq) in [
        (queue_type, "task/t1", 1),
        ("urn:ads:event:task.noted:1", "queue/queue-01", 7),
        (queue_type, "queue/", 1),
    ] {
        let error = raw_append(&fixture.db(), kind, stream, seq).unwrap_err();
        assert!(error.contains("queue"), "{kind} {stream}: {error}");
    }
    // The binding refuses only the family mismatch: the row below passes it (and the chain
    // trigger), and only the authenticated read would refuse its copied ciphertext.
    assert_eq!(
        raw_append(&fixture.db(), queue_type, "queue/queue-01", 7),
        Ok(1)
    );
}

#[test]
fn an_older_file_with_an_unbound_queue_row_is_left_as_it_was() {
    for version in [4, 5] {
        an_unbound_queue_row_is_left_from(version);
    }
}

fn an_unbound_queue_row_is_left_from(version: i64) {
    let fixture = Fixture::new();
    let anchor = MemoryAnchor::default();
    let mut log = open(&fixture.db(), &anchor, "").unwrap();
    log.append(&queue_events()[0]).unwrap();
    drop(log);
    back_to_version(&fixture.db(), version);
    assert_eq!(
        raw_append(&fixture.db(), "urn:ads:event:queue.enabled:1", "task/t1", 1),
        Ok(1)
    );

    let before = {
        let c = rusqlite::Connection::open(fixture.db()).unwrap();
        crate::migrate::schema(&c).unwrap()
    };
    assert!(matches!(
        open(&fixture.db(), &anchor, ""),
        Err(StorageError::UnsupportedSchema)
    ));
    assert_eq!(user_version(&fixture.db()), version);
    let c = rusqlite::Connection::open(fixture.db()).unwrap();
    assert_eq!(crate::migrate::schema(&c).unwrap(), before);
    assert_eq!(
        c.query_row("SELECT count(*) FROM log_genesis", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
