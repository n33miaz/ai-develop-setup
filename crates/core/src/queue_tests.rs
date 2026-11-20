use super::*;
use EventQueueStoppedV1DataVia::{LocalUi, RemoteDevice, Tray};

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn device(text: &str, via: EventQueueStoppedV1DataVia) -> AuthenticatedDevice {
    AuthenticatedDevice { id: id(text), via }
}

fn task() -> RecordQueueTaskV1 {
    serde_json::from_str(include_str!(
        "../../../contracts/golden/record.queue-task.v1/valid/01-base.json"
    ))
    .unwrap()
}

#[test]
fn queue_changes_require_the_authenticated_owner_and_enable_is_local() {
    let owner = device("owner-01", LocalUi);
    let remote = device("owner-01", RemoteDevice);
    let other = device("owner-02", Tray);
    let queue = new_stopped(id("queue-01"), &owner);
    assert_eq!(queue.state, Stopped);
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
    assert_eq!(actor.id(), &queue.owner_id);
    assert_eq!(actor.via(), LocalUi);
    assert_eq!(restore_stopped(enabled.clone()).state, Stopped);
    assert_eq!(
        change_queue_state(&enabled, None, Stopped).err(),
        Some(QueueRefusal::Unauthenticated)
    );
    assert_eq!(
        change_queue_state(&enabled, Some(&other), Stopped).err(),
        Some(QueueRefusal::NotOwner)
    );
    assert_eq!(
        change_queue_state(&enabled, Some(&owner), Stopped)
            .unwrap()
            .0
            .state,
        Stopped
    );
}

#[test]
fn task_transitions_cover_every_state_at_each_attempt_boundary() {
    let owner = device("owner-01", LocalUi);
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
    let owner = device("owner-01", LocalUi);
    let other = device("owner-02", LocalUi);
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
    let different_queue = serde_json::from_value(alternate).unwrap();
    for (candidate, limit, expected) in [
        (&base, 2, false),
        (&duplicate, 2, false),
        (&independent, 2, true),
        (&independent, 1, false),
        (&different_queue, 1, true),
    ] {
        assert_eq!(can_admit_task(candidate, [&base], limit), expected);
    }
    assert!(!can_admit_task(&base, [], 0));
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
