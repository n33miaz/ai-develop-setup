use super::*;
use Channel::{LocalUi, RemoteDevice, Tray};

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn device(device: &str, owner: &str, via: Channel) -> AuthenticatedDevice {
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
    let (enabled, event, actor) = change_queue_state(&queue, Some(&owner), Enabled).unwrap();
    assert_eq!(enabled.state, Enabled);
    assert_eq!(
        event,
        QueueStateEvent::Enabled(EventQueueEnabledV1Data {
            queue_id: id("queue-01"),
            owner_id: id("owner-01"),
            via: EventQueueEnabledV1DataVia::LocalUi,
        })
    );
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
    let (stopped, event, by) = change_queue_state(&enabled, Some(&remote), Stopped).unwrap();
    assert_eq!(stopped.state, Stopped);
    assert_eq!(by.device_id(), &id("phone-01"));
    assert_eq!(
        event,
        QueueStateEvent::Stopped(EventQueueStoppedV1Data {
            queue_id: id("queue-01"),
            reason: EventQueueStoppedV1DataReason::Owner,
            owner_id: Some(id("owner-01")),
            via: Some(EventQueueStoppedV1DataVia::RemoteDevice),
        })
    );
}

#[test]
fn an_owner_stop_is_never_recorded_for_a_stopped_queue() {
    let owner = device("device-01", "owner-01", LocalUi);
    let stopped = new_stopped(id("queue-01"), &owner);
    for device in [&owner, &device("phone-01", "owner-01", RemoteDevice)] {
        assert_eq!(
            change_queue_state(&stopped, Some(device), Stopped).err(),
            Some(QueueRefusal::Unchanged)
        );
    }
}

#[test]
fn the_core_channel_maps_to_each_event_via() {
    assert_eq!(LocalUi.created_via(), EventQueueCreatedV1DataVia::LocalUi);
    assert_eq!(
        RemoteDevice.created_via(),
        EventQueueCreatedV1DataVia::RemoteDevice
    );
    assert_eq!(Tray.enabled_via(), Some(EventQueueEnabledV1DataVia::Tray));
    assert_eq!(RemoteDevice.enabled_via(), None);
    assert_eq!(Tray.stopped_via(), EventQueueStoppedV1DataVia::Tray);
    assert_eq!(
        RemoteDevice.task_via(),
        EventQueueTaskStateChangedV1DataVia::RemoteDevice
    );
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

fn event<T: serde::de::DeserializeOwned>(
    kind: &str,
    seq: u64,
    actor: (&str, &str),
    data: serde_json::Value,
) -> T {
    serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB",
        "type": format!("urn:ads:event:queue.{kind}:1"),
        "ts": "2030-01-02T03:04:05Z",
        "stream": "queue/queue-01",
        "seq": seq,
        "actor": { "kind": actor.0, "id": actor.1 },
        "cause": null,
        "data_class": "internal",
        "data": data,
    }))
    .unwrap()
}

const DAEMON: (&str, &str) = ("daemon", "core-01");

/// The stream of queue-01 owned by owner-01, whose devices are device-01 and phone-01.
struct Log {
    created: EventQueueCreatedV1,
    enabled: EventQueueEnabledV1,
    stopped: EventQueueStoppedV1,
    admitted: EventQueueTaskCreatedV1,
    changes: Vec<EventQueueTaskStateChangedV1>,
}

impl Log {
    fn new(attempt_limit: u64) -> Self {
        let created = event(
            "created",
            1,
            ("device", "device-01"),
            serde_json::json!({
                "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui", "state": "stopped"
            }),
        );
        let enabled = event(
            "enabled",
            2,
            ("device", "device-01"),
            serde_json::json!({ "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui" }),
        );
        let stopped = event(
            "stopped",
            5,
            DAEMON,
            serde_json::json!({ "queue_id": "queue-01", "reason": "restart" }),
        );
        let mut admitted = serde_json::to_value(creation()).unwrap();
        admitted["seq"] = 3.into();
        admitted["data"]["attempt_limit"] = attempt_limit.into();
        Self {
            created,
            enabled,
            stopped,
            admitted: serde_json::from_value(admitted).unwrap(),
            changes: Vec::new(),
        }
    }

    fn change(&mut self, actor: (&str, &str), from: &str, to: &str, attempts: u64) {
        let seq = 10 + self.changes.len() as u64;
        let mut data = serde_json::json!({
            "queue_id": "queue-01", "task_id": "task-01", "from": from, "to": to, "attempts": attempts
        });
        if actor.0 == "device" {
            data["via"] = "tray".into();
        }
        self.changes
            .push(event("task_state_changed", seq, actor, data));
    }

    fn events(&self) -> Vec<QueueLogEvent<'_>> {
        let mut events = vec![
            QueueLogEvent::Created(&self.created),
            QueueLogEvent::Enabled(&self.enabled),
            QueueLogEvent::TaskCreated(&self.admitted),
        ];
        events.extend(self.changes.iter().map(QueueLogEvent::TaskStateChanged));
        events
    }

    fn replay(&self, events: Vec<QueueLogEvent<'_>>) -> Option<(CommonQueueTaskStateV1, u64)> {
        replay_task(&id("task-01"), events, registry)
    }
}

fn registry(device: &CommonIdV1) -> Option<CommonIdV1> {
    match device.as_str() {
        "device-01" | "phone-01" => Some(id("owner-01")),
        "device-02" => Some(id("owner-02")),
        _ => None,
    }
}

#[test]
fn replay_rejects_reset_attempts_and_uses_the_creation_ceiling() {
    let mut log = Log::new(1);
    log.change(DAEMON, "queued", "running", 1);
    log.change(DAEMON, "running", "failed", 1);
    assert_eq!(log.replay(log.events()), Some((Failed, 1)));
    let mut reset = Log::new(1);
    reset.change(DAEMON, "queued", "running", 1);
    reset.change(DAEMON, "running", "failed", 0);
    assert_eq!(reset.replay(reset.events()), None);
    log.change(DAEMON, "failed", "queued", 1);
    assert_eq!(log.replay(log.events()), None);
}

#[test]
fn replay_accepts_done_only_from_a_device_of_the_queue_owner() {
    for (actor, expected) in [
        (("device", "phone-01"), Some((Done, 1))),
        (("device", "device-01"), Some((Done, 1))),
        (("device", "device-02"), None),
        (("device", "unknown-01"), None),
    ] {
        let mut log = Log::new(2);
        log.change(DAEMON, "queued", "running", 1);
        log.change(DAEMON, "running", "review", 1);
        log.change(actor, "review", "done", 1);
        assert_eq!(log.replay(log.events()), expected, "{actor:?}");
    }
}

#[test]
fn replay_requires_the_enabled_queue_when_a_task_starts() {
    let mut log = Log::new(2);
    log.change(DAEMON, "queued", "running", 1);
    assert_eq!(log.replay(log.events()), Some((Running, 1)));
    let mut never_enabled = log.events();
    never_enabled.remove(1);
    assert_eq!(log.replay(never_enabled), None);
    let mut restarted = log.events();
    restarted.insert(3, QueueLogEvent::Stopped(&log.stopped));
    assert_eq!(log.replay(restarted), None);
}

#[test]
fn replay_rejects_queue_changes_by_a_device_of_another_owner() {
    let mut log = Log::new(2);
    log.change(DAEMON, "queued", "running", 1);
    let mut enabled = serde_json::to_value(&log.enabled).unwrap();
    enabled["actor"]["id"] = "device-02".into();
    log.enabled = serde_json::from_value(enabled).unwrap();
    assert_eq!(log.replay(log.events()), None);

    let log = Log::new(2);
    let stop: EventQueueStoppedV1 = event(
        "stopped",
        5,
        ("device", "device-02"),
        serde_json::json!({
            "queue_id": "queue-01", "reason": "owner", "owner_id": "owner-01", "via": "tray"
        }),
    );
    let mut events = log.events();
    events.push(QueueLogEvent::Stopped(&stop));
    assert_eq!(log.replay(events), None);
}

#[test]
fn replay_rejects_out_of_order_and_missing_events() {
    let mut log = Log::new(2);
    log.change(DAEMON, "queued", "running", 1);
    let mut events = log.events();
    events.swap(2, 3);
    assert_eq!(log.replay(events), None, "change before creation");
    let mut events = log.events();
    events.remove(0);
    assert_eq!(log.replay(events), None, "no queue creation");
    let events = log.events()[..2].to_vec();
    assert_eq!(log.replay(events), None, "task never created");
}

#[test]
fn a_restart_fails_the_running_task_only_when_the_host_lost_its_process() {
    let mut task = task();
    task.state = Running;
    task.attempts = RecordQueueTaskV1Attempts::new(1).unwrap();
    assert_eq!(restore_running_task(&task, HostProcess::Kept), None);
    let data = restore_running_task(&task, HostProcess::Lost).unwrap();
    assert_eq!((data.from, data.to), (Running, Failed));
    assert_eq!(data.attempts.get(), 1, "no new attempt");
    assert_eq!((&data.queue_id, &data.task_id), (&task.queue_id, &task.id));
    assert_eq!(data.via, None);

    let mut at_ceiling = task.clone();
    at_ceiling.attempts = RecordQueueTaskV1Attempts::new(2).unwrap();
    let failed = restore_running_task(&at_ceiling, HostProcess::Lost).unwrap();
    assert_eq!(failed.attempts.get(), 2);
    for (state, attempts) in [(Queued, 0), (Review, 1), (Running, 0), (Running, 3)] {
        let mut other = task.clone();
        other.state = state;
        other.attempts = RecordQueueTaskV1Attempts::new(attempts).unwrap();
        assert_eq!(restore_running_task(&other, HostProcess::Lost), None);
    }

    let mut log = Log::new(2);
    log.change(DAEMON, "queued", "running", 1);
    let mut change = serde_json::to_value(&log.changes[0]).unwrap();
    change["seq"] = 11.into();
    change["data"] = serde_json::to_value(&data).unwrap();
    log.changes.push(serde_json::from_value(change).unwrap());
    let mut stop = serde_json::to_value(&log.stopped).unwrap();
    stop["seq"] = 10.into();
    let restart: EventQueueStoppedV1 = serde_json::from_value(stop).unwrap();
    let mut first = serde_json::to_value(&log.changes[0]).unwrap();
    first["seq"] = 9.into();
    let start: EventQueueTaskStateChangedV1 = serde_json::from_value(first).unwrap();
    let events = vec![
        QueueLogEvent::Created(&log.created),
        QueueLogEvent::Enabled(&log.enabled),
        QueueLogEvent::TaskCreated(&log.admitted),
        QueueLogEvent::TaskStateChanged(&start),
        QueueLogEvent::Stopped(&restart),
        QueueLogEvent::TaskStateChanged(&log.changes[1]),
    ];
    assert_eq!(log.replay(events), Some((Failed, 1)));
}

fn admitted(task: &str, origin: serde_json::Value, ts: &str) -> EventQueueTaskCreatedV1 {
    let mut value = serde_json::to_value(creation()).unwrap();
    value["data"]["task_id"] = task.into();
    value["data"]["origin"] = origin;
    value["ts"] = ts.into();
    serde_json::from_value(value).unwrap()
}

fn rule(rule: &str, target: &str) -> serde_json::Value {
    serde_json::json!({ "kind": "bug_rule", "rule_id": rule, "target_id": target })
}

fn manual(request: &str) -> serde_json::Value {
    serde_json::json!({ "kind": "manual", "request_id": request })
}

#[test]
fn admission_limits_each_bug_rule_and_the_queue_within_the_window() {
    let policy = QueuePolicy {
        window_seconds: NonZeroU64::new(3600).unwrap(),
        window_admissions: NonZeroUsize::new(3).unwrap(),
        bug_rule_window_admissions: NonZeroUsize::new(2).unwrap(),
        ..policy()
    };
    let history = [
        admitted(
            "task-01",
            rule("rule-01", "target-01"),
            "2030-01-02T03:00:00Z",
        ),
        admitted(
            "task-02",
            rule("rule-01", "target-02"),
            "2030-01-02T03:30:00Z",
        ),
    ];
    let same_rule = admitted(
        "task-03",
        rule("rule-01", "target-03"),
        "2030-01-02T03:59:59Z",
    );
    let other_rule = admitted(
        "task-03",
        rule("rule-02", "target-03"),
        "2030-01-02T03:59:59Z",
    );
    let later = admitted(
        "task-03",
        rule("rule-01", "target-03"),
        "2030-01-02T04:00:00Z",
    );
    assert!(!can_admit_task(&same_rule, &history, &policy));
    assert!(can_admit_task(&other_rule, &history, &policy));
    assert!(
        can_admit_task(&later, &history, &policy),
        "the window clears"
    );

    let full = [
        admitted("task-01", manual("request-01"), "2030-01-02T03:00:00Z"),
        admitted("task-02", manual("request-02"), "2030-01-02T03:10:00Z"),
        admitted("task-03", manual("request-03"), "2030-01-02T03:20:00Z"),
    ];
    let next = admitted("task-04", manual("request-04"), "2030-01-02T03:59:59Z");
    let after = admitted("task-04", manual("request-04"), "2030-01-02T04:00:00Z");
    let skewed = admitted("task-04", manual("request-04"), "2030-01-02T02:00:00Z");
    let next_day = admitted("task-04", manual("request-04"), "2030-01-03T03:20:00Z");
    assert!(!can_admit_task(&next, &full, &policy));
    assert!(can_admit_task(&after, &full, &policy));
    assert!(
        !can_admit_task(&skewed, &full, &policy),
        "later admissions count inside the window"
    );
    assert!(can_admit_task(&next_day, &full, &policy));
}

#[test]
fn epoch_seconds_follow_the_calendar() {
    for (text, seconds) in [
        ("1970-01-01T00:00:00Z", 0),
        ("2000-03-01T00:00:00Z", 951_868_800),
        ("2030-01-02T03:04:05.5Z", 1_893_553_445),
        ("2024-02-29T23:59:60Z", 1_709_251_200),
    ] {
        let ts = CommonTimestampV1::new(text).unwrap();
        assert_eq!(epoch_seconds(&ts), Some(seconds), "{text}");
    }
}

#[test]
fn queue_policy_validates_configured_limits() {
    let policy: QueuePolicy = toml::from_str(DEFAULT_POLICY).unwrap();
    assert_eq!(policy.attempt_limit.get(), 2);
    assert_eq!(policy.admission_limit.get(), 1000);
    assert_eq!(policy.window_seconds.get(), 3600);
    assert_eq!(policy.window_admissions.get(), 10);
    assert_eq!(policy.bug_rule_window_admissions.get(), 3);
    for (key, value) in [
        ("attempt_limit", "0"),
        ("attempt_limit", "11"),
        ("admission_limit", "0"),
        ("window_seconds", "0"),
        ("window_admissions", "0"),
        ("bug_rule_window_admissions", "0"),
    ] {
        let prefix = format!("{key} =");
        let settings: String = DEFAULT_POLICY
            .lines()
            .map(|line| {
                if line.starts_with(&prefix) {
                    format!("{key} = {value}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect();
        assert!(settings.contains(&format!("{key} = {value}\n")), "{key}");
        assert!(toml::from_str::<QueuePolicy>(&settings).is_err(), "{key}");
    }
}
