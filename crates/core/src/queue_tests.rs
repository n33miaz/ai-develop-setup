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
            can_admit_task(candidate, [&base], &limited(admission), &base.ts),
            expected
        );
    }
    assert!(can_admit_task(&base, [], &policy(), &base.ts));
}

fn ts(text: &str) -> CommonTimestampV1 {
    CommonTimestampV1::new(text).unwrap()
}

const DAEMON: (&str, &str) = ("daemon", "core-01");
const OWNER_DEVICE: (&str, &str) = ("device", "device-01");

/// Typed event of the queue stream, owned by the test.
enum Owned {
    Created(EventQueueCreatedV1),
    Enabled(EventQueueEnabledV1),
    Stopped(EventQueueStoppedV1),
    TaskCreated(EventQueueTaskCreatedV1),
    Changed(EventQueueTaskStateChangedV1),
}

impl Owned {
    fn as_log(&self) -> QueueLogEvent<'_> {
        match self {
            Self::Created(event) => QueueLogEvent::Created(event),
            Self::Enabled(event) => QueueLogEvent::Enabled(event),
            Self::Stopped(event) => QueueLogEvent::Stopped(event),
            Self::TaskCreated(event) => QueueLogEvent::TaskCreated(event),
            Self::Changed(event) => QueueLogEvent::TaskStateChanged(event),
        }
    }
}

/// The stream of queue-01 owned by owner-01, as JSON until `numbered` gives each event its
/// `seq` (from 1) and an instant `seq` seconds after 03:04:00.
struct Stream(Vec<serde_json::Value>);

impl Stream {
    fn new() -> Self {
        Self(Vec::new()).push(
            "created",
            OWNER_DEVICE,
            serde_json::json!({
                "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui", "state": "stopped"
            }),
        )
    }

    fn push(mut self, kind: &str, actor: (&str, &str), data: serde_json::Value) -> Self {
        self.0.push(serde_json::json!({
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB",
            "type": format!("urn:ads:event:queue.{kind}:1"),
            "stream": "queue/queue-01",
            "actor": { "kind": actor.0, "id": actor.1 },
            "cause": null,
            "data_class": "internal",
            "data": data,
        }));
        self
    }

    fn enabled(self, actor: (&str, &str)) -> Self {
        self.push(
            "enabled",
            actor,
            serde_json::json!({ "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui" }),
        )
    }

    fn restart(self) -> Self {
        self.push(
            "stopped",
            DAEMON,
            serde_json::json!({ "queue_id": "queue-01", "reason": "restart" }),
        )
    }

    fn owner_stop(self, actor: (&str, &str), owner: &str) -> Self {
        self.push(
            "stopped",
            actor,
            serde_json::json!({
                "queue_id": "queue-01", "reason": "owner", "owner_id": owner, "via": "tray"
            }),
        )
    }

    fn admit(self, task: &str, limit: u64) -> Self {
        let mut data = serde_json::to_value(creation()).unwrap()["data"].clone();
        data["task_id"] = task.into();
        data["origin"]["request_id"] = format!("request-{task}").into();
        data["attempt_limit"] = limit.into();
        self.push("task_created", DAEMON, data)
    }

    fn change(self, actor: (&str, &str), from: &str, to: &str, attempts: u64) -> Self {
        self.change_of("task-01", actor, from, to, attempts)
    }

    fn change_of(
        self,
        task: &str,
        actor: (&str, &str),
        from: &str,
        to: &str,
        attempts: u64,
    ) -> Self {
        let mut data = serde_json::json!({
            "queue_id": "queue-01", "task_id": task, "from": from, "to": to, "attempts": attempts
        });
        if actor.0 == "device" {
            data["via"] = "tray".into();
        }
        self.push("task_state_changed", actor, data)
    }

    fn started(self) -> Self {
        self.enabled(OWNER_DEVICE)
            .admit("task-01", 2)
            .change(DAEMON, "queued", "running", 1)
    }

    fn numbered(self) -> Vec<serde_json::Value> {
        let mut values = self.0;
        for (index, value) in values.iter_mut().enumerate() {
            let seq = index as u64 + 1;
            value["seq"] = seq.into();
            value["ts"] = format!("2030-01-02T03:{:02}:{:02}Z", 4 + seq / 60, seq % 60).into();
        }
        values
    }

    fn replay(self) -> Result<(CommonQueueTaskStateV1, u64), ReplayRefusal> {
        replay_values(self.numbered())
    }
}

fn typed(values: Vec<serde_json::Value>) -> Vec<Owned> {
    values
        .into_iter()
        .map(|value| {
            let kind = value["type"].as_str().unwrap().to_owned();
            let parse = || value.clone();
            match kind.as_str() {
                "urn:ads:event:queue.created:1" => {
                    Owned::Created(serde_json::from_value(parse()).unwrap())
                }
                "urn:ads:event:queue.enabled:1" => {
                    Owned::Enabled(serde_json::from_value(parse()).unwrap())
                }
                "urn:ads:event:queue.stopped:1" => {
                    Owned::Stopped(serde_json::from_value(parse()).unwrap())
                }
                "urn:ads:event:queue.task_created:1" => {
                    Owned::TaskCreated(serde_json::from_value(parse()).unwrap())
                }
                _ => Owned::Changed(serde_json::from_value(parse()).unwrap()),
            }
        })
        .collect()
}

fn replay_values(
    values: Vec<serde_json::Value>,
) -> Result<(CommonQueueTaskStateV1, u64), ReplayRefusal> {
    let events = typed(values);
    replay_task(&id("task-01"), events.iter().map(Owned::as_log), registry)
}

/// device-01 belongs to owner-01; phone-01 too, until it is revoked at 03:04:07; device-02
/// belongs to owner-02; moved-01 moves from owner-02 to owner-01 at 03:04:03.
fn registry(device: &CommonIdV1, at: &CommonTimestampV1) -> Option<CommonIdV1> {
    let at = at.as_str();
    match device.as_str() {
        "device-01" => Some(id("owner-01")),
        "phone-01" if at < "2030-01-02T03:04:07Z" => Some(id("owner-01")),
        "device-02" => Some(id("owner-02")),
        "moved-01" if at < "2030-01-02T03:04:03Z" => Some(id("owner-02")),
        "moved-01" => Some(id("owner-01")),
        _ => None,
    }
}

#[test]
fn replay_rejects_reset_attempts_and_uses_the_creation_ceiling() {
    let start = || {
        Stream::new()
            .enabled(OWNER_DEVICE)
            .admit("task-01", 1)
            .change(DAEMON, "queued", "running", 1)
    };
    let failed = || start().change(DAEMON, "running", "failed", 1);
    assert_eq!(failed().replay(), Ok((Failed, 1)));
    assert_eq!(
        start().change(DAEMON, "running", "failed", 0).replay(),
        Err(ReplayRefusal::InvalidChange)
    );
    assert_eq!(
        failed().change(DAEMON, "failed", "queued", 1).replay(),
        Err(ReplayRefusal::InvalidChange)
    );
}

#[test]
fn replay_accepts_done_only_from_a_device_of_the_queue_owner() {
    for (actor, expected) in [
        (("device", "phone-01"), Ok((Done, 1))),
        (OWNER_DEVICE, Ok((Done, 1))),
        (("device", "device-02"), Err(ReplayRefusal::NotOwnerDevice)),
        (("device", "unknown-01"), Err(ReplayRefusal::NotOwnerDevice)),
    ] {
        let stream = Stream::new()
            .started()
            .change(DAEMON, "running", "review", 1)
            .change(actor, "review", "done", 1);
        assert_eq!(stream.replay(), expected, "{actor:?}");
    }
}

#[test]
fn replay_requires_the_enabled_queue_when_a_task_starts() {
    assert_eq!(Stream::new().started().replay(), Ok((Running, 1)));
    let never_enabled = Stream::new()
        .admit("task-01", 2)
        .change(DAEMON, "queued", "running", 1);
    assert_eq!(never_enabled.replay(), Err(ReplayRefusal::QueueStopped));
    let restarted = Stream::new()
        .enabled(OWNER_DEVICE)
        .admit("task-01", 2)
        .restart()
        .change(DAEMON, "queued", "running", 1);
    assert_eq!(restarted.replay(), Err(ReplayRefusal::QueueStopped));
    let enabled_again = Stream::new()
        .enabled(OWNER_DEVICE)
        .admit("task-01", 2)
        .restart()
        .enabled(OWNER_DEVICE)
        .change(DAEMON, "queued", "running", 1);
    assert_eq!(enabled_again.replay(), Ok((Running, 1)));
}

#[test]
fn replay_refuses_a_log_with_a_missing_event() {
    let mut values = Stream::new()
        .enabled(OWNER_DEVICE)
        .admit("task-01", 2)
        .restart()
        .change(DAEMON, "queued", "running", 1)
        .numbered();
    assert_eq!(
        replay_values(values.clone()),
        Err(ReplayRefusal::QueueStopped)
    );
    values.remove(3);
    assert_eq!(replay_values(values), Err(ReplayRefusal::SeqGap));

    let mut repeated = Stream::new().started().numbered();
    repeated[3]["seq"] = 3.into();
    assert_eq!(replay_values(repeated), Err(ReplayRefusal::SeqGap));
    let mut late_start = Stream::new().started().numbered();
    for value in &mut late_start {
        value["seq"] = (value["seq"].as_u64().unwrap() + 1).into();
    }
    assert_eq!(
        replay_values(late_start),
        Err(ReplayRefusal::NotFromCreation)
    );
}

#[test]
fn replay_refuses_a_second_creation_and_a_missing_one() {
    let mut no_queue = Stream::new().started().numbered();
    no_queue.remove(0);
    assert_eq!(replay_values(no_queue), Err(ReplayRefusal::NotFromCreation));
    let twice = Stream::new().started().push(
        "created",
        OWNER_DEVICE,
        serde_json::json!({
            "queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui", "state": "stopped"
        }),
    );
    assert_eq!(twice.replay(), Err(ReplayRefusal::NotFromCreation));
    assert_eq!(
        Stream::new().started().admit("task-01", 2).replay(),
        Err(ReplayRefusal::TaskCreation)
    );
    assert_eq!(
        Stream::new().enabled(OWNER_DEVICE).replay(),
        Err(ReplayRefusal::TaskCreation)
    );
    let unknown_task = Stream::new()
        .enabled(OWNER_DEVICE)
        .change(DAEMON, "queued", "running", 1);
    assert_eq!(unknown_task.replay(), Err(ReplayRefusal::TaskCreation));
}

#[test]
fn replay_refuses_events_of_another_queue_or_stream() {
    let mut other_queue = Stream::new().started().numbered();
    other_queue[2]["data"]["queue_id"] = "queue-02".into();
    other_queue[2]["stream"] = "queue/queue-02".into();
    assert_eq!(replay_values(other_queue), Err(ReplayRefusal::ForeignEvent));
    let mut other_stream = Stream::new().started().numbered();
    other_stream[1]["stream"] = "queue/queue-02".into();
    assert_eq!(
        replay_values(other_stream),
        Err(ReplayRefusal::ForeignEvent)
    );
    let mut created_elsewhere = Stream::new().started().numbered();
    created_elsewhere[0]["stream"] = "queue/queue-02".into();
    assert_eq!(
        replay_values(created_elsewhere),
        Err(ReplayRefusal::ForeignEvent)
    );
}

#[test]
fn replay_refuses_queue_changes_outside_the_owner_devices() {
    let base = || Stream::new().admit("task-01", 2);
    assert_eq!(
        base().enabled(("device", "device-02")).replay(),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    let mut foreign_owner = base().enabled(OWNER_DEVICE).numbered();
    foreign_owner[2]["data"]["owner_id"] = "owner-02".into();
    assert_eq!(
        replay_values(foreign_owner),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    assert_eq!(
        base()
            .owner_stop(("device", "device-02"), "owner-01")
            .replay(),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    assert_eq!(
        base().owner_stop(OWNER_DEVICE, "owner-02").replay(),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    assert_eq!(
        base().owner_stop(OWNER_DEVICE, "owner-01").replay(),
        Ok((Queued, 0))
    );
    let mut created_by_other = Stream::new().admit("task-01", 2).numbered();
    created_by_other[0]["actor"]["id"] = "device-02".into();
    assert_eq!(
        replay_values(created_by_other),
        Err(ReplayRefusal::NotOwnerDevice)
    );
}

#[test]
fn replay_refuses_a_task_change_by_a_device_of_another_owner() {
    let other = ("device", "device-02");
    assert_eq!(
        Stream::new()
            .started()
            .change(other, "running", "cancelled", 1)
            .replay(),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    let review = || {
        Stream::new()
            .started()
            .change(DAEMON, "running", "review", 1)
    };
    assert_eq!(
        review().change(other, "review", "failed", 1).replay(),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    assert_eq!(
        review()
            .change(("agent", "agent-01"), "review", "cancelled", 1)
            .replay(),
        Ok((Cancelled, 1))
    );
}

#[test]
fn replay_reads_the_device_owner_at_the_instant_of_each_event() {
    // moved-01 belongs to owner-02 until 03:04:03, so its enable at seq 2 is refused.
    let early = Stream::new()
        .enabled(("device", "moved-01"))
        .admit("task-01", 2);
    assert_eq!(early.replay(), Err(ReplayRefusal::NotOwnerDevice));
    let late = Stream::new()
        .admit("task-01", 2)
        .admit("task-02", 2)
        .enabled(("device", "moved-01"));
    assert_eq!(late.replay(), Ok((Queued, 0)));

    // phone-01 is revoked at 03:04:07: its acceptance at seq 6 stays valid, at seq 7 it is not.
    let accepted = |padding: bool| {
        let mut stream = Stream::new()
            .started()
            .change(DAEMON, "running", "review", 1);
        if padding {
            stream = stream.admit("task-02", 2);
        }
        stream
            .change(("device", "phone-01"), "review", "done", 1)
            .replay()
    };
    assert_eq!(accepted(false), Ok((Done, 1)));
    assert_eq!(accepted(true), Err(ReplayRefusal::NotOwnerDevice));
}

#[test]
fn replay_lets_a_kept_task_finish_after_a_restart() {
    let kept = Stream::new()
        .started()
        .restart()
        .change(DAEMON, "running", "review", 1);
    assert_eq!(kept.replay(), Ok((Review, 1)));
    let other_task = Stream::new()
        .started()
        .admit("task-02", 2)
        .change_of("task-02", DAEMON, "queued", "running", 1);
    assert_eq!(other_task.replay(), Ok((Running, 1)));
}

#[test]
fn a_restart_keeps_fails_or_cancels_the_running_task() {
    let mut task = task();
    task.state = Running;
    task.attempts = RecordQueueTaskV1Attempts::new(1).unwrap();
    assert_eq!(
        restore_running_task(&task, HostProcess::Kept),
        RunningAfterRestart::Keep
    );
    let RunningAfterRestart::Fail(data) = restore_running_task(&task, HostProcess::Lost) else {
        panic!("a lost process fails the task");
    };
    assert_eq!((data.from, data.to), (Running, Failed));
    assert_eq!(data.attempts.get(), 1, "no new attempt");
    assert_eq!((&data.queue_id, &data.task_id), (&task.queue_id, &task.id));
    assert_eq!(data.via, None);

    let mut at_ceiling = task.clone();
    at_ceiling.attempts = RecordQueueTaskV1Attempts::new(2).unwrap();
    assert!(matches!(
        restore_running_task(&at_ceiling, HostProcess::Lost),
        RunningAfterRestart::Fail(data) if data.attempts.get() == 2
    ));
    for (state, attempts) in [(Queued, 0), (Review, 1), (Failed, 1)] {
        let mut other = task.clone();
        other.state = state;
        other.attempts = RecordQueueTaskV1Attempts::new(attempts).unwrap();
        for host in [HostProcess::Kept, HostProcess::Lost] {
            assert_eq!(
                restore_running_task(&other, host),
                RunningAfterRestart::Keep
            );
        }
    }
    for attempts in [0, 3] {
        let mut inconsistent = task.clone();
        inconsistent.attempts = RecordQueueTaskV1Attempts::new(attempts).unwrap();
        for host in [HostProcess::Kept, HostProcess::Lost] {
            let RunningAfterRestart::Cancel(data) = restore_running_task(&inconsistent, host)
            else {
                panic!("an inconsistent count cancels the task: {attempts} {host:?}");
            };
            assert_eq!((data.from, data.to), (Running, Cancelled));
            assert_eq!(data.attempts.get(), attempts);
        }
    }

    let mut values = Stream::new().started().restart().numbered();
    let mut failed = values[3].clone();
    failed["seq"] = 6.into();
    failed["data"] = serde_json::to_value(&data).unwrap();
    values.push(failed);
    assert_eq!(replay_values(values), Ok((Failed, 1)));
}

fn admitted(task: &str, origin: serde_json::Value, at: &str) -> EventQueueTaskCreatedV1 {
    let mut value = serde_json::to_value(creation()).unwrap();
    value["data"]["task_id"] = task.into();
    value["data"]["origin"] = origin;
    value["ts"] = at.into();
    serde_json::from_value(value).unwrap()
}

fn rule(rule: &str, target: &str) -> serde_json::Value {
    serde_json::json!({ "kind": "bug_rule", "rule_id": rule, "target_id": target })
}

fn manual(request: &str) -> serde_json::Value {
    serde_json::json!({ "kind": "manual", "request_id": request })
}

fn issue(number: u64) -> serde_json::Value {
    serde_json::json!({ "kind": "issue", "ref": number })
}

fn windowed() -> QueuePolicy {
    QueuePolicy {
        window_seconds: NonZeroU64::new(3600).unwrap(),
        window_admissions: NonZeroUsize::new(3).unwrap(),
        manual_window_admissions: NonZeroUsize::new(2).unwrap(),
        bug_rule_window_admissions: NonZeroUsize::new(2).unwrap(),
        ..policy()
    }
}

/// Admits at the candidate's own instant, as a producer stamping with the core clock does.
fn admit_now(candidate: &EventQueueTaskCreatedV1, history: &[EventQueueTaskCreatedV1]) -> bool {
    can_admit_task(candidate, history, &windowed(), &candidate.ts)
}

#[test]
fn admission_limits_each_bug_rule_and_the_automatic_origins_within_the_window() {
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
    assert!(!admit_now(&same_rule, &history));
    assert!(admit_now(&other_rule, &history));
    assert!(admit_now(&later, &history), "the window clears");

    let full = [
        admitted("task-01", issue(1), "2030-01-02T03:00:00Z"),
        admitted(
            "task-02",
            rule("rule-01", "target-01"),
            "2030-01-02T03:10:00Z",
        ),
        admitted(
            "task-03",
            rule("rule-02", "target-01"),
            "2030-01-02T03:20:00Z",
        ),
    ];
    let next = admitted("task-04", issue(4), "2030-01-02T03:59:59Z");
    let after = admitted("task-04", issue(4), "2030-01-02T04:00:00Z");
    let next_day = admitted("task-04", issue(4), "2030-01-03T03:20:00Z");
    assert!(!admit_now(&next, &full));
    assert!(admit_now(&after, &full));
    assert!(admit_now(&next_day, &full));
}

#[test]
fn automatic_origins_never_use_up_the_manual_window() {
    let rules_full = [
        admitted(
            "task-01",
            rule("rule-01", "target-01"),
            "2030-01-02T03:00:00Z",
        ),
        admitted(
            "task-02",
            rule("rule-02", "target-01"),
            "2030-01-02T03:01:00Z",
        ),
        admitted(
            "task-03",
            rule("rule-03", "target-01"),
            "2030-01-02T03:02:00Z",
        ),
    ];
    let request = admitted("task-04", manual("request-04"), "2030-01-02T03:03:00Z");
    assert!(admit_now(&request, &rules_full));
    let manual_full = [
        admitted("task-01", manual("request-01"), "2030-01-02T03:00:00Z"),
        admitted("task-02", manual("request-02"), "2030-01-02T03:01:00Z"),
    ];
    let third = admitted("task-03", manual("request-03"), "2030-01-02T03:02:00Z");
    let automatic = admitted("task-03", issue(3), "2030-01-02T03:02:00Z");
    assert!(!admit_now(&third, &manual_full));
    assert!(admit_now(&automatic, &manual_full));
}

#[test]
fn admission_refuses_a_stamp_ahead_of_the_core_clock_or_behind_the_log() {
    let history = [
        admitted("task-01", issue(1), "2030-01-02T03:00:00Z"),
        admitted("task-02", issue(2), "2030-01-02T03:10:00Z"),
        admitted("task-03", issue(3), "2030-01-02T03:20:00Z"),
    ];
    // A stamp a day ahead would leave the whole history out of the window.
    let ahead = admitted("task-04", issue(4), "2030-01-03T03:20:00Z");
    let clock = ts("2030-01-02T03:30:00Z");
    assert!(!can_admit_task(&ahead, &history, &windowed(), &clock));
    let within_skew = admitted("task-04", issue(4), "2030-01-02T03:30:05Z");
    let past_skew = admitted("task-04", issue(4), "2030-01-02T03:30:06Z");
    let policy = QueuePolicy {
        window_admissions: NonZeroUsize::new(4).unwrap(),
        ..windowed()
    };
    assert!(can_admit_task(&within_skew, &history, &policy, &clock));
    assert!(!can_admit_task(&past_skew, &history, &policy, &clock));
    let behind = admitted("task-04", issue(4), "2030-01-02T03:19:59Z");
    assert!(!can_admit_task(&behind, &history, &policy, &clock));
    let same_instant = admitted("task-04", issue(4), "2030-01-02T03:20:00Z");
    assert!(can_admit_task(&same_instant, &history, &policy, &clock));
}

#[test]
fn the_longest_window_never_clears() {
    let policy = QueuePolicy {
        window_seconds: NonZeroU64::new(u64::MAX).unwrap(),
        window_admissions: NonZeroUsize::new(1).unwrap(),
        ..policy()
    };
    let history = [admitted("task-01", issue(1), "1970-01-01T00:00:00Z")];
    let candidate = admitted("task-02", issue(2), "9999-12-31T23:59:59Z");
    assert!(!can_admit_task(
        &candidate,
        &history,
        &policy,
        &candidate.ts
    ));
    assert!(can_admit_task(&candidate, [], &policy, &candidate.ts));
}

#[test]
fn epoch_seconds_follow_the_calendar() {
    for (text, seconds) in [
        ("1970-01-01T00:00:00Z", 0),
        ("2000-03-01T00:00:00Z", 951_868_800),
        ("2030-01-02T03:04:05.5Z", 1_893_553_445),
        ("2024-02-29T23:59:60Z", 1_709_251_200),
    ] {
        assert_eq!(epoch_seconds(&ts(text)), Some(seconds), "{text}");
    }
}

#[test]
fn queue_policy_validates_configured_limits() {
    let policy: QueuePolicy = toml::from_str(DEFAULT_POLICY).unwrap();
    assert_eq!(policy.attempt_limit.get(), 2);
    assert_eq!(policy.admission_limit.get(), 1000);
    assert_eq!(policy.window_seconds.get(), 3600);
    assert_eq!(policy.window_admissions.get(), 10);
    assert_eq!(policy.manual_window_admissions.get(), 10);
    assert_eq!(policy.bug_rule_window_admissions.get(), 3);
    assert_eq!(policy.clock_skew_seconds, 5);
    for (key, value) in [
        ("attempt_limit", "0"),
        ("attempt_limit", "11"),
        ("admission_limit", "0"),
        ("window_seconds", "0"),
        ("window_admissions", "0"),
        ("manual_window_admissions", "0"),
        ("bug_rule_window_admissions", "0"),
        ("clock_skew_seconds", "-1"),
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

#[test]
fn an_older_policy_file_loads_with_the_bundled_window_limits() {
    let older: QueuePolicy = toml::from_str("attempt_limit = 3\nadmission_limit = 50\n").unwrap();
    let bundled = policy();
    assert_eq!(older.attempt_limit.get(), 3);
    assert_eq!(older.admission_limit.get(), 50);
    assert_eq!(older.window_seconds, bundled.window_seconds);
    assert_eq!(older.window_admissions, bundled.window_admissions);
    assert_eq!(
        older.manual_window_admissions,
        bundled.manual_window_admissions
    );
    assert_eq!(
        older.bug_rule_window_admissions,
        bundled.bug_rule_window_admissions
    );
    assert_eq!(older.clock_skew_seconds, bundled.clock_skew_seconds);
    assert!(toml::from_str::<QueuePolicy>("attempt_limit = 3\n").is_err());
}
