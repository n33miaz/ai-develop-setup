use super::*;
use Channel::{LocalUi, RemoteDevice, Tray};
use ads_contracts::{CommonUlidV1, EventQueueTaskStateChangedV2Type};

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

fn task() -> RecordQueueTaskV2 {
    serde_json::from_str(include_str!(
        "../../../contracts/golden/record.queue-task.v2/valid/01-base.json"
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
        EventQueueTaskStateChangedV2DataVia::RemoteDevice
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
                task.attempts = RecordQueueTaskV2Attempts::new(index as u64).unwrap();
                let expected = allowed
                    .iter()
                    .find(|(state, _)| *state == to)
                    .and_then(|(_, counts)| counts[index]);
                assert_eq!(
                    next_attempt_count(&queue, &task, to, Some(&owner))
                        .map(RecordQueueTaskV2Attempts::get),
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
    task.attempts = RecordQueueTaskV2Attempts::new(1).unwrap();
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

/// The golden manual request, created by device-01 of owner-01.
fn creation() -> EventQueueTaskCreatedV1 {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/golden/event.queue.task_created.v1/valid/01-base.json"
    ))
    .unwrap();
    value["actor"] = serde_json::json!({ "kind": "device", "id": "device-01" });
    serde_json::from_value(value).unwrap()
}

/// Admission asked by device-01 of owner-01, the owner of every queue in these tests.
fn admit_as_owner<'a>(
    candidate: &EventQueueTaskCreatedV1,
    history: impl IntoIterator<Item = &'a EventQueueTaskCreatedV1>,
    policy: &QueuePolicy,
    now: &CommonTimestampV1,
    anchor: Option<&ClockAnchor>,
) -> Result<(), AdmissionRefusal> {
    let author = device("device-01", "owner-01", LocalUi);
    let owner = id("owner-01");
    let context = AdmissionContext {
        policy,
        now,
        queue_owner: &owner,
        author: Some(&author),
        anchor,
    };
    can_admit_task(candidate, history, &context)
}

#[test]
fn task_admission_uses_the_origin_after_a_retry_or_restart() {
    use AdmissionRefusal::{AdmissionLimit, AttemptCeiling, OriginRepeated, TaskIdUsed};
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
        (&base, 2, Err(TaskIdUsed)),
        (&duplicate, 2, Err(OriginRepeated)),
        (&independent, 2, Ok(())),
        (&independent, 1, Err(AdmissionLimit)),
        (&different_queue, 1, Ok(())),
        (&same_id_other_queue, 2, Err(TaskIdUsed)),
        (&above_ceiling, 2, Err(AttemptCeiling)),
    ] {
        assert_eq!(
            admit_as_owner(candidate, [&base], &limited(admission), &base.ts, None),
            expected
        );
    }
    assert_eq!(admit_as_owner(&base, [], &policy(), &base.ts, None), Ok(()));
}

fn ts(text: &str) -> CommonTimestampV1 {
    CommonTimestampV1::new(text).unwrap()
}

const DAEMON: (&str, &str) = ("daemon", "core-01");
const OWNER_DEVICE: (&str, &str) = ("device", "device-01");

type Owned = QueueStreamEvent;

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

    fn policy_changed(self, policy: &QueuePolicy) -> Self {
        let seq = self.0.len() as u64;
        let owner = device("device-01", "owner-01", LocalUi);
        let data = change_queue_policy(
            &new_stopped(id("queue-01"), &owner),
            Some(&owner),
            policy,
            seq,
            seq,
        )
        .unwrap();
        self.push(
            "policy_changed",
            OWNER_DEVICE,
            serde_json::to_value(data).unwrap(),
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

    /// Manual request of the owner.
    fn admit(self, task: &str, limit: u64) -> Self {
        self.admit_by(OWNER_DEVICE, task, limit, None)
    }

    fn admit_by(
        self,
        actor: (&str, &str),
        task: &str,
        limit: u64,
        origin: Option<serde_json::Value>,
    ) -> Self {
        let mut data = serde_json::to_value(creation()).unwrap()["data"].clone();
        data["task_id"] = task.into();
        data["origin"]["request_id"] = format!("request-{task}").into();
        if let Some(origin) = origin {
            data["origin"] = origin;
        }
        data["attempt_limit"] = limit.into();
        self.push("task_created", actor, data)
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
            value["id"] = stream_event_id(seq).as_str().into();
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
            let envelope: EventEnvelopeV1 = serde_json::from_value(value).unwrap();
            QueueStreamEvent::from_envelope(&envelope).unwrap()
        })
        .collect()
}

fn replay_values(
    values: Vec<serde_json::Value>,
) -> Result<(CommonQueueTaskStateV1, u64), ReplayRefusal> {
    let events = typed(values);
    replay_events(&events, &policy()).map(|task| (task.state, task.attempts))
}

fn stream_event_id(seq: u64) -> CommonUlidV1 {
    CommonUlidV1::new(format!("01ARZ3NDEKTSV4RR{seq:010}")).unwrap()
}

fn replay_events(events: &[Owned], policy: &QueuePolicy) -> Result<ReplayedTask, ReplayRefusal> {
    replay_with_policy(events, Some(policy))
}

fn replay_with_policy(
    events: &[Owned],
    policy: Option<&QueuePolicy>,
) -> Result<ReplayedTask, ReplayRefusal> {
    replay_task(
        &id("task-01"),
        events.iter().map(Owned::as_log),
        registry,
        policy,
    )
}

#[test]
fn policy_changes_require_the_authenticated_owner_and_current_sequence() {
    let owner = device("device-01", "owner-01", RemoteDevice);
    let other = device("device-02", "owner-02", Tray);
    let queue = new_stopped(id("queue-01"), &owner);
    let configured = policy();
    for (actor, seq, refusal) in [
        (None, 5, QueueRefusal::Unauthenticated),
        (Some(&other), 5, QueueRefusal::NotOwner),
        (Some(&owner), 4, QueueRefusal::StaleSeq),
    ] {
        assert_eq!(
            change_queue_policy(&queue, actor, &configured, seq, 5),
            Err(refusal)
        );
    }
    let data = change_queue_policy(&queue, Some(&owner), &configured, 5, 5).unwrap();
    assert_eq!(data.queue_id, queue.id);
    assert_eq!(data.owner_id, owner.owner_id);
    assert_eq!(data.via, EventQueuePolicyChangedV1DataVia::RemoteDevice);
    assert_eq!(QueuePolicy::from_event(&data.policy), Some(configured));
}

#[test]
fn replay_requires_an_owner_policy_event_to_raise_the_ceiling() {
    let initial = policy();
    let raised = QueuePolicy {
        attempt_limit: ads_contracts::RecordQueueTaskV2AttemptLimit::new(3).unwrap(),
        ..policy()
    };
    let origin = serde_json::json!({"kind": "issue", "ref": 42});
    let stream = Stream::new().admit_by(DAEMON, "task-01", 3, Some(origin.clone()));
    assert_eq!(
        replay_events(&typed(stream.numbered()), &initial),
        Err(ReplayRefusal::Admission(AdmissionRefusal::AttemptCeiling))
    );

    let authorized = Stream::new()
        .policy_changed(&raised)
        .admit_by(DAEMON, "task-01", 3, Some(origin))
        .numbered();
    assert_eq!(
        replay_events(&typed(authorized.clone()), &initial)
            .unwrap()
            .policy,
        raised
    );

    let mut unauthorized = authorized;
    unauthorized[1]["actor"]["id"] = "device-02".into();
    assert_eq!(
        replay_events(&typed(unauthorized), &initial),
        Err(ReplayRefusal::NotOwnerDevice)
    );
}

#[test]
fn replay_uses_the_creation_timestamp_and_requires_a_policy() {
    let mut events = Stream::new().admit("task-01", 2).numbered();
    events[1]["ts"] = "2031-01-02T03:04:02Z".into();
    let events = typed(events);
    assert_eq!(
        replay_with_policy(&events, None),
        Err(ReplayRefusal::MissingInitialPolicy)
    );
    assert_eq!(replay_events(&events, &policy()).unwrap().state, Queued);
}

#[test]
fn manual_creation_v2_checks_the_owner_and_reserves_v1_origins() {
    let mut stream = Stream::new()
        .admit("task-01", 2)
        .admit("task-02", 2)
        .numbered();
    stream[2]["type"] = "urn:ads:event:queue.task_created:2".into();
    assert_eq!(replay_values(stream.clone()), Ok((Queued, 0)));

    for kind in ["daemon", "agent"] {
        let mut unauthorized = stream.clone();
        unauthorized[1]["actor"] = serde_json::json!({"kind": kind, "id": "core-01"});
        assert_eq!(
            replay_values(unauthorized),
            Err(ReplayRefusal::NotOwnerDevice)
        );
    }

    stream[2]["data"]["origin"] = stream[1]["data"]["origin"].clone();
    let earlier: EventQueueTaskCreatedV1 = serde_json::from_value(stream[1].clone()).unwrap();
    let later: EventQueueTaskCreatedV2 = serde_json::from_value(stream[2].clone()).unwrap();
    let author = device("device-01", "owner-01", Tray);
    let context = AdmissionContext {
        policy: &policy(),
        now: &later.ts,
        queue_owner: &id("owner-01"),
        author: Some(&author),
        anchor: None,
    };
    assert_eq!(can_admit_creation((&later).into(), [], &context), Ok(()));
    let stranger = device("device-01", "owner-02", Tray);
    for author in [None, Some(&stranger)] {
        let context = AdmissionContext { author, ..context };
        assert_eq!(
            can_admit_creation((&later).into(), [], &context),
            Err(AdmissionRefusal::ManualNotOwnerDevice)
        );
    }
    assert_eq!(
        can_admit_creation((&later).into(), [(&earlier).into()], &context),
        Err(AdmissionRefusal::OriginRepeated)
    );
    assert_eq!(
        replay_values(stream),
        Err(ReplayRefusal::Admission(AdmissionRefusal::OriginRepeated))
    );
}

#[test]
fn creation_v3_reads_the_shared_origin_and_alias_alongside_older_versions() {
    let mut stream = Stream::new()
        .admit("task-01", 2)
        .admit("task-02", 2)
        .admit("task-03", 2)
        .numbered();
    stream[2]["type"] = "urn:ads:event:queue.task_created:2".into();
    stream[3]["type"] = "urn:ads:event:queue.task_created:3".into();
    assert_eq!(replay_values(stream.clone()), Ok((Queued, 0)));

    let mut foreign = stream.clone();
    foreign[3]["actor"] = serde_json::json!({"kind": "device", "id": "device-02"});
    assert_eq!(replay_values(foreign), Err(ReplayRefusal::NotOwnerDevice));

    let mut other_alias = stream.clone();
    other_alias[3]["data"]["origin"] = stream[2]["data"]["origin"].clone();
    other_alias[3]["data"]["workspace_alias"] = "site".into();
    assert_eq!(replay_values(other_alias), Ok((Queued, 0)));

    for earlier in [1, 2] {
        let mut repeated = stream.clone();
        repeated[3]["data"]["origin"] = stream[earlier]["data"]["origin"].clone();
        assert_eq!(
            replay_values(repeated.clone()),
            Err(ReplayRefusal::Admission(AdmissionRefusal::OriginRepeated)),
            "origin of the creation at seq {}",
            earlier + 1
        );
        let later: EventQueueTaskCreatedV3 = serde_json::from_value(repeated[3].clone()).unwrap();
        let author = device("device-01", "owner-01", Tray);
        let context = AdmissionContext {
            policy: &policy(),
            now: &later.ts,
            queue_owner: &id("owner-01"),
            author: Some(&author),
            anchor: None,
        };
        let events = typed(repeated[1..3].to_vec());
        let history = events
            .iter()
            .filter_map(|event| event.as_log().task_creation());
        assert_eq!(
            can_admit_creation((&later).into(), history, &context),
            Err(AdmissionRefusal::OriginRepeated)
        );
    }
}

#[test]
fn replay_recovers_a_running_task_after_limits_are_lowered() {
    let mut historical = windowed();
    historical.attempt_limit = ads_contracts::RecordQueueTaskV2AttemptLimit::new(3).unwrap();
    let lowered = one_manual();
    let stream = Stream::new()
        .policy_changed(&historical)
        .enabled(OWNER_DEVICE)
        .admit("task-01", 3)
        .admit("task-02", 2)
        .change(DAEMON, "queued", "running", 1)
        .restart()
        .policy_changed(&lowered);
    let events = typed(Stream(stream.0.clone()).numbered());
    let restored = replay_with_policy(&events, None).unwrap();
    assert_eq!((restored.state, restored.attempts), (Running, 1));
    assert_eq!(restored.policy, lowered);

    let events = typed(stream.admit("task-03", 2).numbered());
    assert_eq!(
        replay_with_policy(&events, None),
        Err(ReplayRefusal::Admission(AdmissionRefusal::WindowFull))
    );
}

#[test]
fn replay_restores_the_owner_anchor_and_admission_after_restart() {
    let policy = policy();
    let jumped = "2031-01-02T03:04:02Z";
    let corrected = "2030-01-02T03:04:03Z";
    let first = manual_at("task-01", jumped, 2);
    let second = manual_at("task-02", corrected, 4);
    assert_eq!(
        admit_as_owner(&first, &[], &policy, &first.ts, None),
        Ok(())
    );
    assert_eq!(
        admit_as_owner(
            &second,
            std::slice::from_ref(&first),
            &policy,
            &second.ts,
            None
        ),
        Err(AdmissionRefusal::BehindLastAdmission)
    );

    let persisted = {
        let anchor = owner_anchor(corrected, 2);
        assert_eq!(
            admit_as_owner(
                &second,
                std::slice::from_ref(&first),
                &policy,
                &second.ts,
                Some(&anchor)
            ),
            Ok(())
        );
        let mut values = Stream::new()
            .admit("task-01", 2)
            .push(
                "clock_reanchored",
                OWNER_DEVICE,
                serde_json::to_value(anchor.event_data()).unwrap(),
            )
            .admit("task-02", 2)
            .restart()
            .numbered();
        values[1]["ts"] = jumped.into();
        values[2]["ts"] = anchor.at().as_str().into();
        values[3]["ts"] = corrected.into();
        serde_json::to_vec(&values).unwrap()
    };

    let values: Vec<serde_json::Value> = serde_json::from_slice(&persisted).unwrap();
    let mut missing = values.clone();
    missing.remove(2);
    for (index, value) in missing.iter_mut().enumerate().skip(2) {
        value["seq"] = (index + 1).into();
    }
    assert_eq!(
        replay_events(&typed(missing), &policy),
        Err(ReplayRefusal::Admission(
            AdmissionRefusal::BehindLastAdmission
        ))
    );
    let mut wrong_owner = values.clone();
    wrong_owner[2]["data"]["owner_id"] = "owner-02".into();
    assert_eq!(
        replay_events(&typed(wrong_owner), &policy),
        Err(ReplayRefusal::NotOwnerDevice)
    );
    let restored = replay_events(&typed(values), &policy).unwrap();
    assert_eq!((restored.state, restored.attempts), (Queued, 0));
    let third = manual_at("task-03", "2030-01-02T03:04:06Z", 6);
    let ahead = manual_at("task-03", "2030-01-02T03:04:09Z", 6);
    assert_eq!(
        admit_as_owner(
            &ahead,
            &[first.clone(), second.clone()],
            &policy,
            &ts(corrected),
            restored.anchor.as_ref()
        ),
        Err(AdmissionRefusal::AheadOfClock)
    );
    assert_eq!(
        admit_as_owner(
            &third,
            &[first, second],
            &policy,
            &third.ts,
            restored.anchor.as_ref()
        ),
        Ok(())
    );
}

#[test]
fn clock_recovery_reason_takes_precedence_over_a_full_window() {
    let recent = manual_at("task-01", "2030-01-02T03:29:00Z", 2);
    let ahead = manual_at("task-02", "2031-01-02T03:30:00Z", 3);
    let next = manual_at("task-03", "2030-01-02T03:30:00Z", 4);
    let history = [recent, ahead];
    let policy = one_manual();
    assert_eq!(
        admit_as_owner(&next, &history, &policy, &next.ts, None),
        Err(AdmissionRefusal::BehindLastAdmission)
    );
    let anchor = owner_anchor("2030-01-02T03:30:00Z", 3);
    assert_eq!(
        admit_anchored(&next, &history, &policy, &anchor),
        Err(AdmissionRefusal::WindowFull)
    );
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
fn replay_refuses_a_manual_request_outside_the_owner_devices() {
    let agent = ("agent", "agent-01");
    for actor in [DAEMON, agent, ("device", "device-02")] {
        let own = Stream::new().admit_by(actor, "task-01", 2, None);
        assert_eq!(
            own.replay(),
            Err(ReplayRefusal::NotOwnerDevice),
            "{actor:?}"
        );
        let other_task = Stream::new()
            .admit("task-01", 2)
            .admit_by(actor, "task-02", 2, None);
        assert_eq!(
            other_task.replay(),
            Err(ReplayRefusal::NotOwnerDevice),
            "{actor:?}"
        );
    }
    let from_issue = Stream::new().admit_by(
        DAEMON,
        "task-01",
        2,
        Some(serde_json::json!({ "kind": "issue", "ref": 1 })),
    );
    assert_eq!(from_issue.replay(), Ok((Queued, 0)));
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
fn a_running_task_waits_for_the_host_and_is_abandoned_after_the_deadline() {
    let mut task = task();
    task.state = Running;
    task.attempts = RecordQueueTaskV2Attempts::new(1).unwrap();
    let deadline = 17;
    let policy = QueuePolicy {
        host_confirm_seconds: NonZeroU64::new(deadline).unwrap(),
        ..policy()
    };
    for waited_seconds in [0, deadline - 1] {
        assert_eq!(
            restore_running_task(&task, HostProcess::Unconfirmed { waited_seconds }, &policy),
            RunningAfterRestart::AwaitHost,
        );
    }
    let RunningAfterRestart::Abandon(data) = restore_running_task(
        &task,
        HostProcess::Unconfirmed {
            waited_seconds: deadline,
        },
        &policy,
    ) else {
        panic!("the configured deadline abandons the task");
    };
    assert_eq!(
        (data.from, data.to, data.attempts.get()),
        (Running, Cancelled, 1)
    );
    assert_eq!(
        data.reason,
        Some(EventQueueTaskStateChangedV2DataReason::HostUnconfirmed)
    );
    let stream = Stream::new().started().restart().push(
        "task_state_changed",
        DAEMON,
        serde_json::to_value(&data).unwrap(),
    );
    let mut stream = stream;
    stream.0[5]["type"] = EventQueueTaskStateChangedV2Type::VALUE.into();
    let mut values = stream.restart().enabled(OWNER_DEVICE).numbered();
    let (state, attempts) = replay_values(values.clone()).unwrap();
    assert_eq!((state, attempts), (Cancelled, 1));
    task.state = state;
    let mut queue = new_stopped(id("queue-01"), &device("device-01", "owner-01", LocalUi));
    queue.state = Enabled;
    assert_eq!(next_attempt_count(&queue, &task, Queued, None), None);
    assert_eq!(next_attempt_count(&queue, &task, Running, None), None);
    assert_eq!(
        restore_running_task(&task, HostProcess::Kept, &policy),
        RunningAfterRestart::Keep
    );

    let mut retry = values[3].clone();
    retry["seq"] = 9.into();
    retry["data"]["from"] = "failed".into();
    retry["data"]["to"] = "queued".into();
    values.push(retry);
    assert_eq!(replay_values(values), Err(ReplayRefusal::InvalidChange));
}

#[test]
fn restart_uses_a_persisted_v1_snapshot_and_its_log() {
    let mut saved: RecordQueueTaskV1 = serde_json::from_str(include_str!(
        "../../../contracts/golden/record.queue-task.v1/valid/01-base.json"
    ))
    .unwrap();
    let mut queue = new_stopped(id("queue-01"), &device("device-01", "owner-01", LocalUi));
    queue.state = Enabled;
    assert_eq!(
        next_attempt_count(&queue, &saved, Running, None)
            .unwrap()
            .get(),
        1
    );

    let (state, attempts) = Stream::new().started().restart().replay().unwrap();
    saved.state = state;
    saved.attempts = ads_contracts::RecordQueueTaskV1Attempts::new(attempts).unwrap();
    assert!(matches!(
        restore_running_task(&saved, HostProcess::Lost, &policy()),
        RunningAfterRestart::Fail(data) if data.from == Running && data.attempts.get() == 1
    ));
}

#[test]
fn a_restart_keeps_fails_or_cancels_the_running_task() {
    let mut task = task();
    task.state = Running;
    task.attempts = RecordQueueTaskV2Attempts::new(1).unwrap();
    assert_eq!(
        restore_running_task(&task, HostProcess::Kept, &policy()),
        RunningAfterRestart::Keep
    );
    let RunningAfterRestart::Fail(data) = restore_running_task(&task, HostProcess::Lost, &policy())
    else {
        panic!("a lost process fails the task");
    };
    assert_eq!((data.from, data.to), (Running, Failed));
    assert_eq!(data.attempts.get(), 1, "no new attempt");
    assert_eq!((&data.queue_id, &data.task_id), (&task.queue_id, &task.id));
    assert_eq!(data.via, None);

    let mut at_ceiling = task.clone();
    at_ceiling.attempts = RecordQueueTaskV2Attempts::new(2).unwrap();
    assert!(matches!(
        restore_running_task(&at_ceiling, HostProcess::Lost, &policy()),
        RunningAfterRestart::Fail(data) if data.attempts.get() == 2
    ));
    for (state, attempts) in [(Queued, 0), (Review, 1), (Failed, 1)] {
        let mut other = task.clone();
        other.state = state;
        other.attempts = RecordQueueTaskV2Attempts::new(attempts).unwrap();
        for host in [
            HostProcess::Kept,
            HostProcess::Lost,
            HostProcess::Unconfirmed { waited_seconds: 0 },
            HostProcess::Unconfirmed { waited_seconds: 60 },
        ] {
            assert_eq!(
                restore_running_task(&other, host, &policy()),
                RunningAfterRestart::Keep
            );
        }
    }
    for attempts in [0, 3] {
        let mut inconsistent = task.clone();
        inconsistent.attempts = RecordQueueTaskV2Attempts::new(attempts).unwrap();
        for host in [
            HostProcess::Kept,
            HostProcess::Lost,
            HostProcess::Unconfirmed { waited_seconds: 0 },
        ] {
            let RunningAfterRestart::Cancel(data) =
                restore_running_task(&inconsistent, host, &policy())
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
    failed["type"] = EventQueueTaskStateChangedV2Type::VALUE.into();
    failed["data"] = serde_json::to_value(&data).unwrap();
    values.push(failed);
    assert_eq!(replay_values(values), Ok((Failed, 1)));
}

/// Admission of `task` at `at`; a manual request comes from device-01, any other origin from
/// the daemon.
fn admitted(task: &str, origin: serde_json::Value, at: &str) -> EventQueueTaskCreatedV1 {
    let mut value = serde_json::to_value(creation()).unwrap();
    if origin["kind"] != "manual" {
        value["actor"] = serde_json::json!({ "kind": "daemon", "id": "core-01" });
    }
    value["data"]["task_id"] = task.into();
    value["data"]["origin"] = origin;
    value["ts"] = at.into();
    serde_json::from_value(value).unwrap()
}

fn with_seq(event: EventQueueTaskCreatedV1, seq: u64) -> EventQueueTaskCreatedV1 {
    let mut value = serde_json::to_value(event).unwrap();
    value["seq"] = seq.into();
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
fn admit_now(
    candidate: &EventQueueTaskCreatedV1,
    history: &[EventQueueTaskCreatedV1],
) -> Result<(), AdmissionRefusal> {
    admit_as_owner(candidate, history, &windowed(), &candidate.ts, None)
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
    assert_eq!(
        admit_now(&same_rule, &history),
        Err(AdmissionRefusal::BugRuleWindowFull)
    );
    assert_eq!(admit_now(&other_rule, &history), Ok(()));
    assert_eq!(admit_now(&later, &history), Ok(()), "the window clears");

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
    assert_eq!(admit_now(&next, &full), Err(AdmissionRefusal::WindowFull));
    assert_eq!(admit_now(&after, &full), Ok(()));
    assert_eq!(admit_now(&next_day, &full), Ok(()));
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
    assert_eq!(admit_now(&request, &rules_full), Ok(()));
    let manual_full = [
        admitted("task-01", manual("request-01"), "2030-01-02T03:00:00Z"),
        admitted("task-02", manual("request-02"), "2030-01-02T03:01:00Z"),
    ];
    let third = admitted("task-03", manual("request-03"), "2030-01-02T03:02:00Z");
    let automatic = admitted("task-03", issue(3), "2030-01-02T03:02:00Z");
    assert_eq!(
        admit_now(&third, &manual_full),
        Err(AdmissionRefusal::WindowFull)
    );
    assert_eq!(admit_now(&automatic, &manual_full), Ok(()));
}

#[test]
fn a_manual_request_needs_a_device_of_the_queue_owner() {
    let request = admitted("task-01", manual("request-01"), "2030-01-02T03:00:00Z");
    let by = |kind: &str, actor: &str| {
        let mut value = serde_json::to_value(&request).unwrap();
        value["actor"] = serde_json::json!({ "kind": kind, "id": actor });
        serde_json::from_value::<EventQueueTaskCreatedV1>(value).unwrap()
    };
    let policy = policy();
    let owner = id("owner-01");
    let check = |candidate: &EventQueueTaskCreatedV1, author: Option<&AuthenticatedDevice>| {
        let context = AdmissionContext {
            policy: &policy,
            now: &candidate.ts,
            queue_owner: &owner,
            author,
            anchor: None,
        };
        can_admit_task(candidate, [], &context)
    };
    let owner_device = device("device-01", "owner-01", LocalUi);
    let other_owner = device("device-02", "owner-02", LocalUi);
    let other_device = device("phone-01", "owner-01", RemoteDevice);
    let refused = Err(AdmissionRefusal::ManualNotOwnerDevice);
    assert_eq!(check(&request, Some(&owner_device)), Ok(()));
    assert_eq!(check(&request, None), refused);
    assert_eq!(check(&request, Some(&other_owner)), refused);
    assert_eq!(check(&request, Some(&other_device)), refused, "actor id");
    for (kind, actor) in [("daemon", "device-01"), ("agent", "device-01")] {
        assert_eq!(check(&by(kind, actor), Some(&owner_device)), refused);
    }
    let from_issue = admitted("task-01", issue(1), "2030-01-02T03:00:00Z");
    assert_eq!(check(&from_issue, None), Ok(()));
}

#[test]
fn admission_refuses_a_stamp_ahead_of_the_core_clock_or_behind_the_log() {
    use AdmissionRefusal::{AheadOfClock, BehindLastAdmission};
    let history = [
        admitted("task-01", issue(1), "2030-01-02T03:00:00Z"),
        admitted("task-02", issue(2), "2030-01-02T03:10:00Z"),
        admitted("task-03", issue(3), "2030-01-02T03:20:00Z"),
    ];
    // A stamp a day ahead would leave the whole history out of the window.
    let ahead = admitted("task-04", issue(4), "2030-01-03T03:20:00Z");
    let clock = ts("2030-01-02T03:30:00Z");
    let admit = |candidate: &EventQueueTaskCreatedV1, policy: &QueuePolicy| {
        admit_as_owner(candidate, &history, policy, &clock, None)
    };
    assert_eq!(admit(&ahead, &windowed()), Err(AheadOfClock));
    let within_skew = admitted("task-04", issue(4), "2030-01-02T03:30:05Z");
    let past_skew = admitted("task-04", issue(4), "2030-01-02T03:30:06Z");
    let policy = QueuePolicy {
        window_admissions: NonZeroUsize::new(4).unwrap(),
        ..windowed()
    };
    assert_eq!(admit(&within_skew, &policy), Ok(()));
    assert_eq!(admit(&past_skew, &policy), Err(AheadOfClock));
    let within_rewind = admitted("task-04", issue(4), "2030-01-02T03:15:00Z");
    let past_rewind = admitted("task-04", issue(4), "2030-01-02T03:14:59Z");
    assert_eq!(admit(&within_rewind, &policy), Ok(()));
    assert_eq!(admit(&past_rewind, &policy), Err(BehindLastAdmission));
    let same_instant = admitted("task-04", issue(4), "2030-01-02T03:20:00Z");
    assert_eq!(admit(&same_instant, &policy), Ok(()));
    let no_rewind = QueuePolicy {
        clock_rewind_seconds: 0,
        ..policy
    };
    let behind = admitted("task-04", issue(4), "2030-01-02T03:19:59Z");
    assert_eq!(admit(&behind, &no_rewind), Err(BehindLastAdmission));
    // Within the rewind, a later prior still fills the window of the candidate.
    let full = QueuePolicy {
        window_admissions: NonZeroUsize::new(3).unwrap(),
        ..no_rewind
    };
    let rewound = QueuePolicy {
        clock_rewind_seconds: 300,
        ..full
    };
    assert_eq!(
        admit(&within_rewind, &rewound),
        Err(AdmissionRefusal::WindowFull)
    );
}

fn manual_at(task: &str, at: &str, seq: u64) -> EventQueueTaskCreatedV1 {
    with_seq(admitted(task, manual(&format!("request-{task}")), at), seq)
}

fn one_manual() -> QueuePolicy {
    QueuePolicy {
        manual_window_admissions: NonZeroUsize::new(1).unwrap(),
        ..windowed()
    }
}

/// owner-01 re-anchors queue-01 from device-01 at `at`, through `seq`, the last of the stream.
fn owner_anchor(at: &str, seq: u64) -> ClockAnchor {
    let owner = device("device-01", "owner-01", Tray);
    let queue = new_stopped(id("queue-01"), &owner);
    reanchor_admission_clock(&queue, Some(&owner), &ts(at), seq, seq).unwrap()
}

/// Admits at the candidate's own instant with `anchor`.
fn admit_anchored(
    candidate: &EventQueueTaskCreatedV1,
    history: &[EventQueueTaskCreatedV1],
    policy: &QueuePolicy,
    anchor: &ClockAnchor,
) -> Result<(), AdmissionRefusal> {
    admit_as_owner(candidate, history, policy, &candidate.ts, Some(anchor))
}

#[test]
fn the_owner_reanchors_the_queue_after_the_core_clock_ran_ahead() {
    use AdmissionRefusal::{BehindLastAdmission, ForeignAnchor, OriginRepeated, WindowFull};
    let owner = device("device-01", "owner-01", Tray);
    let queue = new_stopped(id("queue-01"), &owner);
    let policy = windowed();
    // Admitted while the core clock read 2031, then the clock is corrected.
    let ahead = manual_at("task-01", "2031-06-01T00:00:00Z", 2);
    assert_eq!(admit_as_owner(&ahead, [], &policy, &ahead.ts, None), Ok(()));
    let history = [ahead];
    let now = ts("2030-01-02T03:30:00Z");
    let next = manual_at("task-02", "2030-01-02T03:30:00Z", 4);
    assert_eq!(
        admit_as_owner(&next, &history, &policy, &now, None),
        Err(BehindLastAdmission),
        "the queue stays refused until the owner acts"
    );

    assert_eq!(
        reanchor_admission_clock(&queue, None, &now, 2, 2),
        Err(QueueRefusal::Unauthenticated)
    );
    let stranger = device("device-02", "owner-02", Tray);
    assert_eq!(
        reanchor_admission_clock(&queue, Some(&stranger), &now, 2, 2),
        Err(QueueRefusal::NotOwner)
    );
    for (through, last) in [(3, 2), (u64::MAX, 2), (1, 2)] {
        assert_eq!(
            reanchor_admission_clock(&queue, Some(&owner), &now, through, last),
            Err(QueueRefusal::StaleSeq),
            "{through} {last}"
        );
    }
    let anchor = reanchor_admission_clock(&queue, Some(&owner), &now, 2, 2).unwrap();
    assert_eq!(
        (anchor.queue_id(), anchor.owner_id(), anchor.device_id()),
        (&id("queue-01"), &id("owner-01"), &id("device-01"))
    );
    assert_eq!(
        (anchor.via(), anchor.at(), anchor.through_seq()),
        (Tray, &now, 2)
    );
    assert_eq!(admit_anchored(&next, &history, &policy, &anchor), Ok(()));

    // The anchored admission still counts, at the anchor, and keeps its origin.
    assert_eq!(
        admit_anchored(&next, &history, &one_manual(), &anchor),
        Err(WindowFull)
    );
    let window_later = manual_at("task-02", "2030-01-02T04:30:00Z", 4);
    assert_eq!(
        admit_anchored(&window_later, &history, &one_manual(), &anchor),
        Ok(())
    );
    let mut same_origin = serde_json::to_value(&next).unwrap();
    same_origin["data"]["origin"]["request_id"] = "request-task-01".into();
    let same_origin = serde_json::from_value(same_origin).unwrap();
    assert_eq!(
        admit_anchored(&same_origin, &history, &policy, &anchor),
        Err(OriginRepeated)
    );

    // An admission after the anchor is not moved by it.
    let after_anchor = [
        history[0].clone(),
        manual_at("task-03", "2031-06-01T00:00:00Z", 3),
    ];
    assert_eq!(
        admit_anchored(&next, &after_anchor, &policy, &anchor),
        Err(BehindLastAdmission)
    );

    for (queue_id, owner_id) in [("queue-02", "owner-01"), ("queue-01", "owner-02")] {
        let mut foreign = anchor.clone();
        foreign.queue_id = id(queue_id);
        foreign.owner_id = id(owner_id);
        assert_eq!(
            admit_as_owner(&next, &history, &policy, &now, Some(&foreign)),
            Err(ForeignAnchor),
            "{queue_id} {owner_id}"
        );
    }
}

#[test]
fn an_anchor_never_covers_the_candidate_or_later_admissions() {
    use AdmissionRefusal::{AnchorAfterCandidate, WindowFull};
    let ahead = manual_at("task-01", "2031-06-01T00:00:00Z", 2);
    let anchor = owner_anchor("2030-01-02T03:30:00Z", 2);
    let mut beyond = anchor.clone();
    beyond.through_seq = u64::MAX;

    // Requests every 20 minutes for three hours: the legitimate anchor lets one in per hour,
    // and one that covers the rest of the stream is refused at once.
    let mut history = vec![ahead];
    let mut admitted_at = Vec::new();
    for step in 0..9u64 {
        let minutes = 31 + step * 20;
        let at = format!("2030-01-02T{:02}:{:02}:00Z", 3 + minutes / 60, minutes % 60);
        let seq = history.len() as u64 + 2;
        let candidate = manual_at(&format!("task-{seq:02}"), &at, seq);
        assert_eq!(
            admit_anchored(&candidate, &history, &one_manual(), &beyond),
            Err(AnchorAfterCandidate),
            "{at}"
        );
        match admit_anchored(&candidate, &history, &one_manual(), &anchor) {
            Ok(()) => {
                admitted_at.push(minutes);
                history.push(candidate);
            }
            refusal => assert_eq!(refusal, Err(WindowFull), "{at}"),
        }
    }
    assert_eq!(
        admitted_at,
        [91, 151],
        "one request per window after the anchor"
    );

    let covering = owner_anchor("2030-01-02T03:30:00Z", 3);
    let candidate = manual_at("task-03", "2030-01-02T03:31:00Z", 3);
    assert_eq!(
        admit_anchored(&candidate, &history[..1], &windowed(), &covering),
        Err(AnchorAfterCandidate)
    );
}

#[test]
fn an_anchor_ahead_of_the_clock_is_refused_and_an_old_one_keeps_the_window() {
    use AdmissionRefusal::{AnchorAheadOfClock, WindowFull};
    let ahead = manual_at("task-01", "2031-06-01T00:00:00Z", 2);
    let candidate = manual_at("task-02", "2030-01-02T03:30:00Z", 3);
    let early = owner_anchor("2031-01-01T00:00:00Z", 2);
    assert_eq!(
        admit_anchored(
            &candidate,
            std::slice::from_ref(&ahead),
            &windowed(),
            &early
        ),
        Err(AnchorAheadOfClock)
    );
    let within_skew = owner_anchor("2030-01-02T03:30:05Z", 2);
    assert_eq!(
        admit_anchored(
            &candidate,
            std::slice::from_ref(&ahead),
            &windowed(),
            &within_skew
        ),
        Ok(())
    );

    // An old instant does not move ordinary creations out of the window.
    let ordinary = [
        manual_at("task-01", "2030-01-02T03:00:00Z", 2),
        manual_at("task-02", "2030-01-02T03:10:00Z", 3),
    ];
    let old = owner_anchor("1970-01-01T00:00:00Z", 3);
    let third = manual_at("task-03", "2030-01-02T03:20:00Z", 4);
    assert_eq!(
        admit_anchored(&third, &ordinary, &windowed(), &old),
        Err(WindowFull)
    );
}

#[test]
fn a_second_anchor_covers_the_creations_of_the_first() {
    use AdmissionRefusal::{BehindLastAdmission, WindowFull};
    let first = owner_anchor("2030-01-02T03:30:00Z", 2);
    let mut history = vec![manual_at("task-01", "2031-06-01T00:00:00Z", 2)];
    let after_first = manual_at("task-03", "2030-01-02T03:30:00Z", 3);
    assert_eq!(
        admit_anchored(&after_first, &history, &windowed(), &first),
        Ok(())
    );
    history.push(after_first);
    // The clock runs ahead again, and is corrected again.
    history.push(manual_at("task-04", "2032-01-01T00:00:00Z", 4));
    let next = manual_at("task-05", "2030-01-02T03:50:00Z", 5);
    let wide = QueuePolicy {
        manual_window_admissions: NonZeroUsize::new(4).unwrap(),
        ..windowed()
    };
    assert_eq!(
        admit_anchored(&next, &history, &wide, &first),
        Err(BehindLastAdmission)
    );
    let second = owner_anchor("2030-01-02T03:50:00Z", 4);
    assert_eq!(admit_anchored(&next, &history, &wide, &second), Ok(()));
    let three = QueuePolicy {
        manual_window_admissions: NonZeroUsize::new(3).unwrap(),
        ..windowed()
    };
    assert_eq!(
        admit_anchored(&next, &history, &three, &second),
        Err(WindowFull),
        "the creations of both jumps still fill the window"
    );
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
    assert_eq!(
        admit_as_owner(&candidate, &history, &policy, &candidate.ts, None),
        Err(AdmissionRefusal::WindowFull)
    );
    assert_eq!(
        admit_as_owner(&candidate, [], &policy, &candidate.ts, None),
        Ok(())
    );
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
    assert_eq!(policy.clock_rewind_seconds, 300);
    assert_eq!(policy.host_confirm_seconds.get(), 60);
    for (key, value) in [
        ("attempt_limit", "0"),
        ("attempt_limit", "11"),
        ("admission_limit", "0"),
        ("window_seconds", "0"),
        ("window_admissions", "0"),
        ("manual_window_admissions", "0"),
        ("bug_rule_window_admissions", "0"),
        ("clock_skew_seconds", "-1"),
        ("clock_rewind_seconds", "-1"),
        ("host_confirm_seconds", "0"),
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
    assert_eq!(older.clock_rewind_seconds, bundled.clock_rewind_seconds);
    assert_eq!(older.host_confirm_seconds, bundled.host_confirm_seconds);
    assert!(toml::from_str::<QueuePolicy>("attempt_limit = 3\n").is_err());
}

#[test]
fn task_change_channel_rules_are_versioned() {
    let mut legacy = Stream::new()
        .started()
        .change(OWNER_DEVICE, "running", "cancelled", 1)
        .numbered();
    let mut cancelled = legacy[4].clone();
    legacy[4]["data"].as_object_mut().unwrap().remove("via");
    assert_eq!(replay_values(legacy.clone()), Ok((Cancelled, 1)));

    let parse = serde_json::from_value::<EventQueueTaskStateChangedV2>;
    let mut missing_channel = legacy[4].clone();
    missing_channel["type"] = EventQueueTaskStateChangedV2Type::VALUE.into();
    assert!(parse(missing_channel).is_err());

    cancelled["type"] = EventQueueTaskStateChangedV2Type::VALUE.into();
    let mut fields = parse(cancelled.clone()).unwrap().into_fields();
    fields.actor.kind = CommonActorV1Kind::Daemon;
    assert!(EventQueueTaskStateChangedV2::new(fields).is_err());

    legacy[4] = cancelled;
    assert_eq!(replay_values(legacy), Ok((Cancelled, 1)));
}

#[test]
fn every_queue_event_version_decodes_from_its_envelope() {
    let goldens = [
        include_str!("../../../contracts/golden/event.queue.created.v1/valid/01-base.json"),
        include_str!("../../../contracts/golden/event.queue.enabled.v1/valid/01-base.json"),
        include_str!("../../../contracts/golden/event.queue.stopped.v1/valid/04-restart.json"),
        include_str!(
            "../../../contracts/golden/event.queue.clock_reanchored.v1/valid/01-base.json"
        ),
        include_str!(
            "../../../contracts/golden/event.queue.policy_changed.v1/valid/02-lowered-limits.json"
        ),
        include_str!("../../../contracts/golden/event.queue.task_created.v1/valid/01-base.json"),
        include_str!("../../../contracts/golden/event.queue.task_created.v2/valid/01-base.json"),
        include_str!("../../../contracts/golden/event.queue.task_created.v3/valid/01-base.json"),
        include_str!("../../../contracts/golden/event.queue.task_created.v4/valid/01-base.json"),
        include_str!(
            "../../../contracts/golden/event.queue.task_state_changed.v1/valid/01-base.json"
        ),
        include_str!(
            "../../../contracts/golden/event.queue.task_state_changed.v2/valid/01-start.json"
        ),
    ];
    for golden in goldens {
        let envelope: EventEnvelopeV1 = serde_json::from_str(golden).unwrap();
        let decoded = QueueStreamEvent::from_envelope(&envelope).unwrap();
        assert_eq!(decoded.as_log().stream(), envelope.stream.as_str());
        assert_eq!(decoded.as_log().seq(), envelope.seq.get());
    }

    let mut envelope: EventEnvelopeV1 = serde_json::from_str(goldens[0]).unwrap();
    envelope.r#type =
        ads_contracts::EventEnvelopeV1Type::new("urn:ads:event:queue.created:9").unwrap();
    assert_eq!(
        QueueStreamEvent::from_envelope(&envelope),
        Err(QueueEventRefusal::UnknownType)
    );
    let mut envelope: EventEnvelopeV1 = serde_json::from_str(goldens[0]).unwrap();
    envelope.data.remove("owner_id");
    assert_eq!(
        QueueStreamEvent::from_envelope(&envelope),
        Err(QueueEventRefusal::Invalid)
    );
    let mut envelope: EventEnvelopeV1 = serde_json::from_str(goldens[0]).unwrap();
    envelope.actor.kind = CommonActorV1Kind::Agent;
    assert_eq!(
        QueueStreamEvent::from_envelope(&envelope),
        Err(QueueEventRefusal::Invalid)
    );
}

/// The stream as the storage hands it back after a reopen: envelopes, replayed after decoding,
/// with the owner lowering the policy below what the history already holds.
#[test]
fn a_stored_queue_replays_after_the_policy_is_lowered() {
    let lowered = one_manual();
    let values = Stream::new()
        .policy_changed(&policy())
        .enabled(OWNER_DEVICE)
        .admit("task-01", 2)
        .admit("task-02", 2)
        .change(DAEMON, "queued", "running", 1)
        .policy_changed(&lowered)
        .numbered();
    let events: Vec<QueueStreamEvent> = values
        .into_iter()
        .map(|value| {
            let envelope: EventEnvelopeV1 = serde_json::from_value(value).unwrap();
            QueueStreamEvent::from_envelope(&envelope).unwrap()
        })
        .collect();
    let restored = replay_with_policy(&events, None).unwrap();
    assert_eq!((restored.state, restored.attempts), (Running, 1));
    assert_eq!(restored.policy, lowered);
}

/// Turns the creation at `index` into `queue.task_created:4` with a title and a tool.
fn as_v4(values: &mut [serde_json::Value], index: usize, title: &str, tool: &str) {
    values[index]["type"] = "urn:ads:event:queue.task_created:4".into();
    values[index]["data"]["title"] = title.into();
    values[index]["data"]["tool"] = tool.into();
}

#[test]
fn replay_queue_rebuilds_every_task_record_in_one_pass() {
    let mut values = Stream::new()
        .policy_changed(&policy())
        .enabled(OWNER_DEVICE)
        .admit("task-01", 2)
        .admit("task-02", 1)
        .admit("task-03", 2)
        .change_of("task-02", DAEMON, "queued", "running", 1)
        .change(DAEMON, "queued", "running", 1)
        .change_of("task-02", DAEMON, "running", "failed", 1)
        .change(DAEMON, "running", "review", 1)
        .change(OWNER_DEVICE, "review", "done", 1)
        .numbered();
    values[4]["type"] = "urn:ads:event:queue.task_created:3".into();
    as_v4(&mut values, 5, "Conferir documentação", "tool-a");
    let events = typed(values.clone());

    let queue = replay_queue(events.iter().map(Owned::as_log), registry, None).unwrap();
    let tasks = &queue.tasks;
    assert_eq!(
        tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["task-01", "task-02", "task-03"]
    );
    let summary = |task: &RecordQueueTaskV3| {
        (
            task.state,
            task.attempts.get(),
            task.attempt_limit.get(),
            task.created_at.as_str().to_owned(),
            task.updated_at.as_str().to_owned(),
        )
    };
    let at = |seq: usize| values[seq - 1]["ts"].as_str().unwrap().to_owned();
    assert_eq!(summary(&tasks[0]), (Done, 1, 2, at(4), at(11)));
    assert_eq!(summary(&tasks[1]), (Failed, 1, 1, at(5), at(9)));
    assert_eq!(summary(&tasks[2]), (Queued, 0, 2, at(6), at(6)));

    // Only the fourth version records title and tool; older creations show them as absent.
    assert_eq!(
        (tasks[0].title.as_ref(), tasks[0].tool.as_ref()),
        (None, None)
    );
    assert_eq!(
        (tasks[1].title.as_ref(), tasks[1].tool.as_ref()),
        (None, None)
    );
    assert_eq!(
        tasks[2].title.as_ref().map(CommonQueueTaskTitleV1::as_str),
        Some("Conferir documentação")
    );
    assert_eq!(tasks[2].tool, Some(id("tool-a")));
    for (position, task) in tasks.iter().enumerate() {
        assert_eq!(task.queue_id, id("queue-01"));
        assert_eq!(task.workspace_alias.as_str(), "docs");
        assert!(task.branch.is_none() && task.pull_request.is_none());
        let json = serde_json::to_value(task).unwrap();
        // An older creation writes both keys as an explicit `null`, never leaves them out.
        for key in ["title", "tool"] {
            let value = json.get(key);
            if position < 2 {
                assert_eq!(value, Some(&serde_json::Value::Null), "{key}");
            } else {
                assert!(value.is_some_and(serde_json::Value::is_string), "{key}");
            }
        }
        assert_eq!(
            serde_json::from_value::<RecordQueueTaskV3>(json).unwrap(),
            *task
        );
    }
    assert_eq!(
        serde_json::to_value(&tasks[1]).unwrap()["origin"],
        serde_json::json!({ "kind": "manual", "request_id": "request-task-02" })
    );

    // The one pass agrees with the replay of each task on its own.
    for task in tasks {
        let alone =
            replay_task(&task.id, events.iter().map(Owned::as_log), registry, None).unwrap();
        assert_eq!(
            (alone.state, alone.attempts),
            (task.state, task.attempts.get())
        );
        assert_eq!(alone.policy, queue.policy);
        assert_eq!(alone.anchor, queue.anchor);
    }
}

#[test]
fn replay_queue_applies_the_task_checks_to_every_task() {
    let base = Stream::new()
        .admit("task-01", 2)
        .admit("task-02", 2)
        .change_of("task-02", DAEMON, "queued", "running", 1)
        .numbered();
    let events = typed(base);
    // The single-task replay skips task-02; the one pass refuses its start on a stopped queue.
    assert_eq!(
        replay_events(&events, &policy()).map(|t| t.state),
        Ok(Queued)
    );
    assert_eq!(
        replay_queue(events.iter().map(Owned::as_log), registry, Some(&policy())),
        Err(ReplayRefusal::QueueStopped)
    );

    let never_created = typed(
        Stream::new()
            .enabled(OWNER_DEVICE)
            .admit("task-01", 2)
            .change_of("task-09", DAEMON, "queued", "running", 1)
            .numbered(),
    );
    assert_eq!(
        replay_events(&never_created, &policy()).map(|t| t.state),
        Ok(Queued)
    );
    assert_eq!(
        replay_queue(
            never_created.iter().map(Owned::as_log),
            registry,
            Some(&policy())
        ),
        Err(ReplayRefusal::TaskCreation)
    );

    let mut twice = Stream::new()
        .admit("task-01", 2)
        .admit("task-02", 2)
        .numbered();
    twice[2]["data"]["task_id"] = "task-01".into();
    twice[2]["data"]["origin"]["request_id"] = "request-other".into();
    assert_eq!(
        replay_queue(
            typed(twice).iter().map(Owned::as_log),
            registry,
            Some(&policy())
        ),
        Err(ReplayRefusal::TaskCreation)
    );

    // A task created twice under distinct requests refuses the stream for every task, also
    // when the replay follows another one.
    let mut twice = Stream::new()
        .admit("task-01", 2)
        .admit("task-02", 2)
        .admit("task-03", 2)
        .numbered();
    twice[2]["data"]["task_id"] = "task-01".into();
    let twice = typed(twice);
    for task in ["task-01", "task-02", "task-03"] {
        assert_eq!(
            replay_task(
                &id(task),
                twice.iter().map(Owned::as_log),
                registry,
                Some(&policy())
            )
            .map(|t| t.state),
            Err(ReplayRefusal::TaskCreation),
            "{task}"
        );
    }
    assert_eq!(
        replay_queue(twice.iter().map(Owned::as_log), registry, Some(&policy())),
        Err(ReplayRefusal::TaskCreation)
    );

    let empty = typed(Stream::new().numbered());
    let queue = replay_queue(empty.iter().map(Owned::as_log), registry, Some(&policy())).unwrap();
    assert!(queue.tasks.is_empty());
}

#[test]
fn a_clock_rewind_never_dates_the_record_update_before_its_creation() {
    // (creation ts, change ts, expected updated_at)
    for (created, changed, updated) in [
        (
            "2030-01-02T03:04:03Z",
            "2030-01-02T03:04:01Z",
            "2030-01-02T03:04:03Z",
        ),
        // A shorter text is not an earlier instant: `05Z` sorts after `05.5Z` as text.
        (
            "2030-01-02T03:04:03.5Z",
            "2030-01-02T03:04:03Z",
            "2030-01-02T03:04:03.5Z",
        ),
        (
            "2030-01-02T03:04:03.5Z",
            "2030-01-02T03:04:03.500001Z",
            "2030-01-02T03:04:03.500001Z",
        ),
    ] {
        let mut values = Stream::new()
            .enabled(OWNER_DEVICE)
            .admit("task-01", 2)
            .change(DAEMON, "queued", "running", 1)
            .numbered();
        values[2]["ts"] = created.into();
        values[3]["ts"] = changed.into();
        let events = typed(values);
        let queue =
            replay_queue(events.iter().map(Owned::as_log), registry, Some(&policy())).unwrap();
        let task = &queue.tasks[0];
        assert_eq!(task.state, Running);
        assert_eq!(task.created_at.as_str(), created);
        assert_eq!(task.updated_at.as_str(), updated, "{created} {changed}");
    }
}

#[test]
fn creation_v4_is_admitted_with_the_older_versions_and_a_v3_record_restores() {
    let mut values = Stream::new()
        .enabled(OWNER_DEVICE)
        .admit("task-01", 2)
        .numbered();
    as_v4(&mut values, 2, "Corrigir issue 42", "tool-b");
    let events = typed(values.clone());
    let mut record = replay_queue(events.iter().map(Owned::as_log), registry, Some(&policy()))
        .unwrap()
        .tasks
        .remove(0);
    assert_eq!(record.tool, Some(id("tool-b")));

    let mut foreign = values;
    foreign[2]["actor"] = serde_json::json!({"kind": "agent", "id": "agent-01"});
    let envelope: EventEnvelopeV1 = serde_json::from_value(foreign[2].clone()).unwrap();
    assert_eq!(
        QueueStreamEvent::from_envelope(&envelope),
        Err(QueueEventRefusal::Invalid)
    );

    record.state = Running;
    record.attempts = RecordQueueTaskV3Attempts::new(1).unwrap();
    assert!(matches!(
        restore_running_task(&record, HostProcess::Lost, &policy()),
        RunningAfterRestart::Fail(data) if data.from == Running && data.attempts.get() == 1
    ));
}

#[test]
fn the_local_owner_acts_only_through_the_local_interface() {
    let local = AuthenticatedDevice::local_owner(id("device-01"), id("owner-01"));
    assert_eq!(local.via(), LocalUi);
    let queue = new_stopped(id("queue-01"), &local);
    let (_, event, _) = change_queue_state(&queue, Some(&local), Enabled).unwrap();
    assert!(matches!(
        event,
        QueueStateEvent::Enabled(data) if data.via == EventQueueEnabledV1DataVia::LocalUi
    ));
    let stranger = AuthenticatedDevice::local_owner(id("device-02"), id("owner-02"));
    assert_eq!(
        change_queue_state(&queue, Some(&stranger), Enabled).err(),
        Some(QueueRefusal::NotOwner)
    );
}
