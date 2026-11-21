//! A queue written to the anchored log, read back after reopening and replayed into task records.

use ads_contracts::{CommonIdV1, CommonQueueTaskStateV1, CommonTimestampV1, EventEnvelopeV1};
use ads_core::queue::{
    QueuePolicy, QueueStreamEvent, new_local_stopped, replay_queue, replay_task,
};
use ads_daemon::queue::{LoggedQueue, QueueOwner};
use ads_storage::{
    EventLog, StorageConfig, StorageError,
    anchor::{AnchorStore, Commitment, DatabaseId},
    cipher::{CipherError, KeyId, KeyProvider, SecretKey},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// A database folder inside the repository's `.test-tmp`, removed file by file on drop.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        assert!(root.ends_with(".test-tmp"));
        let dir = root.join(format!("{name}-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self { dir }
    }

    fn db(&self) -> PathBuf {
        self.dir.join("audit.db")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in [
            "queue.toml",
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

struct Keys;

impl KeyProvider for Keys {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        Ok((KeyId::new("key-a")?, SecretKey::new(&[1; 32])))
    }

    fn by_id(&self, _: &KeyId) -> Result<SecretKey, CipherError> {
        Ok(SecretKey::new(&[1; 32]))
    }
}

/// The vault slot, kept across reopenings like the real one.
#[derive(Clone, Default)]
struct Anchor(Arc<Mutex<Option<Commitment>>>);

impl AnchorStore for Anchor {
    fn can_create(&self) -> Result<bool, StorageError> {
        Ok(self.0.lock().unwrap().is_none())
    }

    fn read(&self, _: DatabaseId) -> Result<Option<Commitment>, StorageError> {
        Ok(*self.0.lock().unwrap())
    }

    fn store(&self, _: DatabaseId, commitment: &Commitment) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(*commitment);
        Ok(())
    }
}

fn open(path: &Path, anchor: &Anchor) -> EventLog {
    EventLog::open(
        path,
        &StorageConfig::defaults(),
        Box::new(Keys),
        Box::new(anchor.clone()),
    )
    .unwrap()
}

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn owner_at(device: &CommonIdV1, _: &CommonTimestampV1) -> Option<CommonIdV1> {
    (device.as_str() == "device-01").then(|| id("owner-01"))
}

const DEVICE: (&str, &str) = ("device", "device-01");
const DAEMON: (&str, &str) = ("daemon", "core-01");

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

fn creation(task: &str, version: u8, details: Option<(&str, &str)>) -> serde_json::Value {
    let mut data = serde_json::json!({
        "queue_id": "queue-01", "task_id": task, "attempt_limit": 2,
        "origin": { "kind": "manual", "request_id": format!("request-{task}") },
        "workspace_alias": "docs", "state": "queued", "attempts": 0
    });
    if let Some((title, tool)) = details {
        data["title"] = title.into();
        data["tool"] = tool.into();
    }
    assert_eq!(details.is_some(), version == 4);
    data
}

fn change(task: &str, from: &str, to: &str, via: Option<&str>) -> serde_json::Value {
    let mut data = serde_json::json!({
        "queue_id": "queue-01", "task_id": task, "from": from, "to": to, "attempts": 1
    });
    if let Some(via) = via {
        data["via"] = via.into();
    }
    data
}

/// Created, policy, enabled, one task of each creation version, the third one done.
fn queue_events() -> Vec<EventEnvelopeV1> {
    let owner =
        serde_json::json!({ "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui" });
    let mut created = owner.clone();
    created["state"] = "stopped".into();
    let mut policy = owner.clone();
    policy["policy"] = serde_json::json!({
        "attempt_limit": 2, "admission_limit": 1000, "window_seconds": 3600,
        "window_admissions": 10, "manual_window_admissions": 10,
        "bug_rule_window_admissions": 3, "clock_skew_seconds": 5,
        "clock_rewind_seconds": 300, "host_confirm_seconds": 60
    });
    vec![
        envelope(1, "created:1", DEVICE, created),
        envelope(2, "policy_changed:1", DEVICE, policy),
        envelope(3, "enabled:1", DEVICE, owner),
        envelope(4, "task_created:1", DEVICE, creation("task-v1", 1, None)),
        envelope(5, "task_created:2", DEVICE, creation("task-v2", 2, None)),
        envelope(6, "task_created:3", DEVICE, creation("task-01", 3, None)),
        envelope(
            7,
            "task_created:4",
            DEVICE,
            creation("task-02", 4, Some(("Conferir documentação", "tool-a"))),
        ),
        envelope(
            8,
            "task_state_changed:2",
            DAEMON,
            change("task-01", "queued", "running", None),
        ),
        envelope(
            9,
            "task_state_changed:2",
            DAEMON,
            change("task-01", "running", "review", None),
        ),
        envelope(
            10,
            "task_state_changed:2",
            DEVICE,
            change("task-01", "review", "done", Some("local_ui")),
        ),
    ]
}

#[test]
fn queue_records_are_rebuilt_from_the_log_after_reopening() {
    let fixture = Fixture::new("queue-records");
    let anchor = Anchor::default();
    let events = queue_events();
    let mut log = open(&fixture.db(), &anchor);
    for event in &events {
        log.append(event).unwrap();
    }
    drop(log);

    let log = open(&fixture.db(), &anchor);
    let stored: Vec<QueueStreamEvent> = log
        .read_queue(&id("queue-01"))
        .unwrap()
        .iter()
        .map(|stored| QueueStreamEvent::from_envelope(&stored.event).unwrap())
        .collect();
    assert_eq!(stored.len(), events.len());

    let queue = replay_queue(stored.iter().map(QueueStreamEvent::as_log), owner_at).unwrap();
    let [first, second, done, queued] = &queue.tasks[..] else {
        panic!("four tasks: {:?}", queue.tasks);
    };
    assert_eq!((&first.id, &second.id), (&id("task-v1"), &id("task-v2")));
    assert_eq!(done.id, id("task-01"));
    assert_eq!(done.state, CommonQueueTaskStateV1::Done);
    assert_eq!(done.attempts.get(), 1);
    assert_eq!(done.created_at.as_str(), "2030-01-02T03:04:06Z");
    assert_eq!(done.updated_at.as_str(), "2030-01-02T03:04:10Z");
    // Versions 1 to 3 did not record title nor tool: the record writes both as an explicit null.
    for older in [first, second, done] {
        assert_eq!(
            older.state,
            if older == done {
                CommonQueueTaskStateV1::Done
            } else {
                CommonQueueTaskStateV1::Queued
            }
        );
        assert!(older.title.is_none() && older.tool.is_none());
        let json = serde_json::to_value(older).unwrap();
        for key in ["title", "tool"] {
            assert_eq!(json.get(key), Some(&serde_json::Value::Null), "{key}");
        }
    }

    assert_eq!(queued.id, id("task-02"));
    assert_eq!(queued.state, CommonQueueTaskStateV1::Queued);
    assert_eq!(queued.attempts.get(), 0);
    assert_eq!(
        queued.title.as_ref().map(|title| title.as_str()),
        Some("Conferir documentação")
    );
    assert_eq!(queued.tool, Some(id("tool-a")));
    for task in &queue.tasks {
        assert_eq!(task.workspace_alias.as_str(), "docs");
        assert!(task.branch.is_none() && task.pull_request.is_none());
        let alone = replay_task(
            &task.id,
            stored.iter().map(QueueStreamEvent::as_log),
            owner_at,
        )
        .unwrap();
        assert_eq!(
            (alone.state, alone.attempts),
            (task.state, task.attempts.get())
        );
        assert_eq!(alone.policy, queue.policy);
    }
}

/// Lowering the policy on reopening is covered with a session in the unit tests of `queue.rs`.
#[test]
fn a_rebound_queue_replays_the_policy_of_its_creation() {
    let fixture = Fixture::new("queue-initial-policy");
    let initial: QueuePolicy =
        toml::from_str("attempt_limit = 3\nadmission_limit = 1000\n").unwrap();
    let anchor = Anchor::default();
    let bind_queue = || {
        LoggedQueue::bind(
            Arc::new(Mutex::new(open(&fixture.db(), &anchor))),
            id("queue-01"),
            QueueOwner {
                owner_id: id("owner-01"),
                device_id: id("device-01"),
            },
        )
        .unwrap()
    };
    let mut log = open(&fixture.db(), &anchor);
    let (_, created) = new_local_stopped(id("queue-01"), &id("owner-01"), &initial);
    let created = serde_json::to_value(created).unwrap();
    log.append(&envelope(1, "created:2", DEVICE, created))
        .unwrap();
    drop(log);
    drop(bind_queue());
    let mut log = open(&fixture.db(), &anchor);
    let mut task = creation("task-01", 1, None);
    task["attempt_limit"] = initial.attempt_limit.get().into();
    log.append(&envelope(2, "task_created:1", DEVICE, task))
        .unwrap();
    drop(log);
    drop(bind_queue());
    let log = open(&fixture.db(), &anchor);
    let stored: Vec<_> = log
        .read_queue(&id("queue-01"))
        .unwrap()
        .iter()
        .map(|stored| QueueStreamEvent::from_envelope(&stored.event).unwrap())
        .collect();
    let queue = replay_queue(stored.iter().map(QueueStreamEvent::as_log), owner_at).unwrap();
    assert_eq!(queue.policy, Some(initial));
    let [task] = &queue.tasks[..] else {
        panic!("expected one task");
    };
    assert_eq!(task.id, id("task-01"));
    assert_eq!(task.state, CommonQueueTaskStateV1::Queued);
    assert_eq!(task.attempts.get(), 0);
    assert_eq!(task.attempt_limit.get(), 3);
}
