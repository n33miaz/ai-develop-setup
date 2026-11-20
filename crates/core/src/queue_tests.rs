use super::*;
use EventQueueStoppedV1DataVia::{LocalUi, RemoteDevice, Tray};

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn device(device: &str, owner: &str, via: EventQueueStoppedV1DataVia) -> AuthenticatedDevice {
    AuthenticatedDevice {
        device_id: id(device),
        owner_id: id(owner),
        via,
    }
}

fn policy() -> QueuePolicy {
    toml::from_str(DEFAULT_POLICY).unwrap()
}

fn task() -> RecordQueueTaskV1 {
    serde_json::from_str(include_str!(
        "../../../contracts/golden/record.queue-task.v1/valid/01-base.json"
    ))
    .unwrap()
}

#[test]
fn queue_changes_require_the_authenticated_owner_and_enable_is_local() {
    let owner = device("device-01", "owner-01", LocalUi);
    let remote = device("phone-01", "owner-01", RemoteDevice);
    let other = device("device-01", "owner-02", Tray);
    let queue = new_stopped(id("queue-01"), &owner);
    assert_eq!(queue.state, Stopped);
    assert_eq!(queue.owner_id, id("owner-01"));
    for (device, state, refusal) in [
        (None, Enabled, QueueRefusal::Unauthenticated),
        (Some(&other), Enabled, QueueRefusal::NotOwner),
        (Some(&remote), Enabled, QueueRefusal::RemoteEnable),
        (Some(&owner), Stopped, QueueRefusal::Unchanged),
    ] {
        assert_eq!(
            change_queue_state(&queue, device, state).err(),
            Some(refusal)
        );
    }
    let (enabled, actor) = change_queue_state(&queue, Some(&owner), Enabled).unwrap();
    assert_eq!(enabled.state, Enabled);
    assert_eq!(actor.device_id(), &id("device-01"));
    assert_eq!(actor.owner_id(), &queue.owner_id);
    assert_eq!(actor.via(), LocalUi);
    assert_eq!(
        change_queue_state(&enabled, None, Stopped).err(),
        Some(QueueRefusal::Unauthenticated)
    );
    assert_eq!(
        change_queue_state(&enabled, Some(&other), Stopped).err(),
        Some(QueueRefusal::NotOwner)
    );
    let (stopped, by) = change_queue_state(&enabled, Some(&remote), Stopped).unwrap();
    assert_eq!(stopped.state, Stopped);
    assert_eq!(
        owner_stop_data(&enabled, &other).err(),
        Some(QueueRefusal::NotOwner)
    );
    let data = owner_stop_data(&enabled, by).unwrap();
    assert_eq!(data.reason, EventQueueStoppedV1DataReason::Owner);
    assert_eq!(data.owner_id, Some(id("owner-01")));
    assert_eq!(data.via, Some(RemoteDevice));
}

#[test]
fn restart_and_automatic_stops_record_the_daemon_reason() {
    let owner = device("device-01", "owner-01", LocalUi);
    let stopped = new_stopped(id("queue-01"), &owner);
    let enabled = change_queue_state(&stopped, Some(&owner), Enabled)
        .unwrap()
        .0;
    let (restored, event) = restore_stopped(enabled.clone(), None);
    assert_eq!(restored.state, Stopped);
    let event = event.unwrap();
    assert_eq!(event.reason, EventQueueStoppedV1DataReason::Restart);
    assert_eq!(event.queue_id, enabled.id);
    assert_eq!((event.owner_id, event.via), (None, None));
    assert_eq!(
        restore_stopped(stopped.clone(), None),
        (stopped.clone(), None)
    );
    assert_eq!(
        restore_stopped(stopped.clone(), Some(Stopped)),
        (stopped.clone(), None)
    );
    let (after_crash, event) = restore_stopped(stopped.clone(), Some(Enabled));
    assert_eq!(after_crash.state, Stopped);
    assert_eq!(
        event.map(|data| data.reason),
        Some(EventQueueStoppedV1DataReason::Restart)
    );
    for (cause, reason) in [
        (AutomaticStop::Budget, EventQueueStoppedV1DataReason::Budget),
        (
            AutomaticStop::Resources,
            EventQueueStoppedV1DataReason::Resources,
        ),
    ] {
        let (after, event) = stop_automatically(&enabled, cause);
        assert_eq!(after.state, Stopped);
        assert_eq!(event.map(|data| data.reason), Some(reason));
    }
}

#[test]
fn task_transitions_cover_every_state_at_each_attempt_boundary() {
    let owner = device("device-01", "owner-01", LocalUi);
    let mut task = task();
    let stopped = new_stopped(task.queue_id.clone(), &owner);
    let queue = change_queue_state(&stopped, Some(&owner), Enabled)
        .unwrap()
        .0;
    let states = [Queued, Running, Review, Failed, Done, Cancelled];
    let transitions: [(CommonQueueTaskStateV1, &[_]); 6] = [
        (
            Queued,
            &[
                (Running, [Some(1), Some(2), None, None]),
                (Cancelled, [Some(0), Some(1), Some(2), Some(3)]),
            ],
        ),
        (
            Running,
            &[
                (Review, [None, Some(1), Some(2), None]),
                (Failed, [None, Some(1), Some(2), None]),
                (Cancelled, [Some(0), Some(1), Some(2), Some(3)]),
            ],
        ),
        (
            Review,
            &[
                (Done, [None, Some(1), Some(2), None]),
                (Failed, [None, Some(1), Some(2), None]),
                (Cancelled, [Some(0), Some(1), Some(2), Some(3)]),
            ],
        ),
        (
            Failed,
            &[
                (Queued, [None, Some(1), None, None]),
                (Cancelled, [Some(0), Some(1), Some(2), Some(3)]),
            ],
        ),
        (Done, &[]),
        (Cancelled, &[]),
    ];
    for (from, allowed) in transitions {
        task.state = from;
        for to in states {
            for index in 0..4 {
                task.attempts = RecordQueueTaskV1Attempts::new(index as u64).unwrap();
                let expected = allowed
                    .iter()
                    .find(|(state, _)| *state == to)
                    .and_then(|(_, counts)| counts[index]);
                assert_eq!(
                    next_attempt_count(&queue, &task, to, Some(&owner))
                        .map(RecordQueueTaskV1Attempts::get),
                    expected,
                    "{from:?} -> {to:?} with {index} attempts"
                );
            }
        }
    }
}

#[test]
fn starting_requires_the_enabled_queue_and_acceptance_requires_its_owner() {
    let owner = device("device-01", "owner-01", LocalUi);
    let phone = device("phone-01", "owner-01", RemoteDevice);
    let other = device("device-01", "owner-02", LocalUi);
    let mut task = task();
    let stopped = new_stopped(task.queue_id.clone(), &owner);
    let mut enabled = change_queue_state(&stopped, Some(&owner), Enabled)
        .unwrap()
        .0;
    assert_eq!(next_attempt_count(&stopped, &task, Running, None), None);
    enabled.id = id("queue-02");
    assert_eq!(next_attempt_count(&enabled, &task, Running, None), None);
    task.state = Review;
    task.attempts = RecordQueueTaskV1Attempts::new(1).unwrap();
    assert_eq!(next_attempt_count(&stopped, &task, Done, None), None);
    assert_eq!(
        next_attempt_count(&stopped, &task, Done, Some(&other)),
        None
    );
    assert_eq!(
        next_attempt_count(&stopped, &task, Done, Some(&owner)),
        Some(task.attempts)
    );
    assert_eq!(
        next_attempt_count(&stopped, &task, Done, Some(&phone)),
        Some(task.attempts)
    );
    task.state = Running;
    for next in [Review, Failed, Cancelled] {
        assert_eq!(
            next_attempt_count(&stopped, &task, next, None),
            Some(task.attempts),
            "a running task finishes its attempt after the queue stops: {next:?}"
        );
    }
}

fn creation() -> EventQueueTaskCreatedV1 {
    serde_json::from_str(include_str!(
        "../../../contracts/golden/event.queue.task_created.v1/valid/01-base.json"
    ))
    .unwrap()
}

#[test]
fn task_admission_uses_the_origin_after_a_retry_or_restart() {
    let base = creation();
    let mut alternate = serde_json::to_value(&base).unwrap();
    alternate["data"]["task_id"] = "task-02".into();
    let duplicate = serde_json::from_value(alternate.clone()).unwrap();
    alternate["data"]["origin"]["request_id"] = "request-02".into();
    let independent = serde_json::from_value(alternate.clone()).unwrap();
    alternate["data"]["queue_id"] = "queue-02".into();
    alternate["stream"] = "queue/queue-02".into();
    alternate["data"]["task_id"] = "task-03".into();
    let different_queue = serde_json::from_value(alternate.clone()).unwrap();
    alternate["data"]["task_id"] = "task-01".into();
    let same_id_other_queue = serde_json::from_value(alternate.clone()).unwrap();
    alternate["data"]["task_id"] = "task-04".into();
    alternate["data"]["attempt_limit"] = 3.into();
    let above_ceiling = serde_json::from_value(alternate).unwrap();
    let limited = |admission: usize| QueuePolicy {
        admission_limit: std::num::NonZeroUsize::new(admission).unwrap(),
        ..policy()
    };
    for (candidate, admission, expected) in [
        (&base, 2, false),
        (&duplicate, 2, false),
        (&independent, 2, true),
        (&independent, 1, false),
        (&different_queue, 1, true),
        (&same_id_other_queue, 2, false),
        (&above_ceiling, 2, false),
    ] {
        assert_eq!(
            can_admit_task(candidate, [&base], &limited(admission)),
            expected
        );
    }
    assert!(can_admit_task(&base, [], &policy()));
}

#[test]
fn replay_rejects_reset_attempts_and_uses_the_creation_ceiling() {
    let mut created = serde_json::to_value(creation()).unwrap();
    created["data"]["attempt_limit"] = 1.into();
    let created = serde_json::from_value(created).unwrap();
    let mut change: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/golden/event.queue.task_state_changed.v1/valid/01-base.json"
    ))
    .unwrap();
    change["seq"] = 2.into();
    let start = serde_json::from_value(change.clone()).unwrap();
    change["seq"] = 3.into();
    change["data"]["from"] = "running".into();
    change["data"]["to"] = "failed".into();
    let fail = serde_json::from_value(change.clone()).unwrap();
    change["data"]["attempts"] = 0.into();
    let reset = serde_json::from_value(change.clone()).unwrap();
    change["seq"] = 4.into();
    change["data"]["from"] = "failed".into();
    change["data"]["to"] = "queued".into();
    change["data"]["attempts"] = 1.into();
    let retry = serde_json::from_value(change).unwrap();
    assert_eq!(
        replay_attempts(&created, [&start, &fail]),
        Some((Failed, 1))
    );
    assert_eq!(replay_attempts(&created, [&start, &reset]), None);
    assert_eq!(replay_attempts(&created, [&start, &fail, &retry]), None);
}

#[test]
fn queue_policy_validates_configured_limits() {
    let policy: QueuePolicy = toml::from_str(DEFAULT_POLICY).unwrap();
    assert_eq!(policy.attempt_limit.get(), 2);
    assert_eq!(policy.admission_limit.get(), 1000);
    for settings in [
        "attempt_limit = 0\nadmission_limit = 1",
        "attempt_limit = 11\nadmission_limit = 1",
        "attempt_limit = 2\nadmission_limit = 0",
    ] {
        assert!(toml::from_str::<QueuePolicy>(settings).is_err());
    }
}
