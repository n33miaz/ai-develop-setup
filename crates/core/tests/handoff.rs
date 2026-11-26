use std::num::NonZeroU64;

use ads_contracts::{
    CommonIdV1, CommonTimestampV1, CommonUlidV1, EventSessionHandoffV3, EventSessionHandoffV4,
    RecordSessionHandoffV2, RecordSessionHandoffV2ConsentChannel as Channel,
    RecordSessionHandoffV2Number, RecordSessionHandoffV2Trigger as Trigger,
};
use ads_core::handoff::{
    ChainError, Consent, ConsentError, ContextUsage, DEFAULT_POLICY, Decision, HandoffPolicy,
    next_session_name, validate_chain, verify_consent,
};
use serde_json::json;

fn usage(used: u64) -> Option<ContextUsage> {
    Some(ContextUsage {
        used,
        capacity: NonZeroU64::new(100).unwrap(),
    })
}

fn policy(mode: &str, pct: u8) -> HandoffPolicy {
    HandoffPolicy::from_toml(&format!("mode = \"{mode}\"\ncontext_threshold_pct = {pct}")).unwrap()
}

#[test]
fn disabled_mode_suppresses_all_triggers() {
    for trigger in [
        Trigger::ContextLimit,
        Trigger::AccountSwitch,
        Trigger::ModelRefusal,
    ] {
        assert_eq!(
            policy("off", 80).evaluate(trigger, usage(100), Consent::Granted),
            Decision::None
        );
    }
}

#[test]
fn default_proposes_at_threshold_and_on_other_triggers() {
    let policy = HandoffPolicy::from_toml(DEFAULT_POLICY).unwrap();
    assert_eq!(
        policy.evaluate(Trigger::ContextLimit, None, Consent::Pending),
        Decision::None
    );
    assert_eq!(
        policy.evaluate(Trigger::ContextLimit, usage(79), Consent::Pending),
        Decision::None
    );
    assert_eq!(
        policy.evaluate(Trigger::ContextLimit, usage(80), Consent::Pending),
        Decision::Propose
    );
    for trigger in [Trigger::AccountSwitch, Trigger::ModelRefusal] {
        assert_eq!(
            policy.evaluate(trigger, None, Consent::Pending),
            Decision::Propose
        );
    }
}

#[test]
fn default_proceeds_after_consent() {
    let policy = HandoffPolicy::from_toml(DEFAULT_POLICY).unwrap();
    for (trigger, usage) in [
        (Trigger::ContextLimit, usage(80)),
        (Trigger::AccountSwitch, None),
        (Trigger::ModelRefusal, None),
    ] {
        assert_eq!(
            policy.evaluate(trigger, usage, Consent::Granted),
            Decision::Proceed
        );
        assert_eq!(
            policy.evaluate(trigger, usage, Consent::Declined),
            Decision::None
        );
    }
}

#[test]
fn automatic_mode_requires_consent_for_every_trigger() {
    let policy = policy("auto-with-consent", 90);
    for (consent, decision) in [
        (Consent::Pending, Decision::Propose),
        (Consent::Granted, Decision::Proceed),
        (Consent::Declined, Decision::None),
    ] {
        assert_eq!(
            policy.evaluate(Trigger::ContextLimit, usage(90), consent),
            decision
        );
        assert_eq!(
            policy.evaluate(Trigger::AccountSwitch, None, consent),
            decision
        );
        assert_eq!(
            policy.evaluate(Trigger::ModelRefusal, None, consent),
            decision
        );
    }
    assert_eq!(
        policy.evaluate(Trigger::ContextLimit, usage(89), Consent::Granted),
        Decision::None
    );
}

#[test]
fn rejects_invalid_settings() {
    for pct in [0, 101] {
        assert!(
            HandoffPolicy::from_toml(&format!(
                "mode = \"propose\"\ncontext_threshold_pct = {pct}"
            ))
            .is_err()
        );
    }
    assert!(HandoffPolicy::from_toml("mode = \"auto\"\ncontext_threshold_pct = 80").is_err());
}

fn chain() -> [RecordSessionHandoffV2; 2] {
    [
        serde_json::from_str(include_str!(
            "../../../contracts/golden/record.session-handoff.v2/valid/01.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../contracts/golden/record.session-handoff.v2/valid/02.json"
        ))
        .unwrap(),
    ]
}

#[test]
fn names_the_next_session_from_the_lineage_and_utc_date() {
    let [first, second] = chain();
    assert_eq!(
        next_session_name(&first.lineage, 1, &first.created_at).unwrap(),
        "project-a n2 · 2030-01-02"
    );
    assert_eq!(
        next_session_name(&second.lineage, first.number.get(), &second.created_at).unwrap(),
        "project-a n3 · 2030-01-02"
    );
    for current in [0, 9_007_199_254_740_991, u64::MAX] {
        assert_eq!(
            next_session_name(&first.lineage, current, &first.created_at),
            Err(ChainError::Number)
        );
    }
}

#[test]
fn validates_the_ordered_golden_chain() {
    assert_eq!(validate_chain(&[]), Ok(()));
    assert_eq!(validate_chain(&chain()), Ok(()));
}

#[test]
fn rejects_cycles_and_broken_links() {
    let mut records = chain();
    records[0].previous_handoff_id = Some(records[1].id.clone());
    assert_eq!(validate_chain(&records), Err(ChainError::PreviousHandoff));

    let mut records = chain();
    records[1].target_session_id = records[0].source_session_id.clone();
    assert_eq!(validate_chain(&records), Err(ChainError::RepeatedSession));

    let mut records = chain();
    records[1].id = records[0].id.clone();
    assert_eq!(validate_chain(&records), Err(ChainError::RepeatedHandoff));

    let mut records = chain();
    records[1].previous_handoff_id = None;
    assert_eq!(validate_chain(&records), Err(ChainError::PreviousHandoff));

    let mut records = chain();
    records[1].source_session_id = CommonIdV1::new("unrelated".to_owned()).unwrap();
    assert_eq!(validate_chain(&records), Err(ChainError::SourceSession));
}

#[test]
fn rejects_lineage_changes() {
    let mut records = chain();
    records[1].lineage = CommonIdV1::new("project-b".to_owned()).unwrap();
    assert_eq!(validate_chain(&records), Err(ChainError::Lineage));
}

#[test]
fn rejects_a_gap_after_the_first_handoff() {
    let mut records = chain();
    records[1].number = RecordSessionHandoffV2Number::new(4).unwrap();
    assert_eq!(validate_chain(&records), Err(ChainError::Number));
}

#[test]
fn rejects_a_gap_before_the_first_handoff() {
    let [mut first, _] = chain();
    first.number = RecordSessionHandoffV2Number::new(3).unwrap();
    assert_eq!(validate_chain(&[first]), Err(ChainError::Number));
}

#[test]
fn rejects_consent_from_another_user() {
    let mut records = chain();
    records[1].consent.user_id = CommonIdV1::new("user-2".to_owned()).unwrap();
    assert_eq!(validate_chain(&records), Err(ChainError::User));
}

fn chain_created_at(first: &str, second: &str) -> [RecordSessionHandoffV2; 2] {
    let mut records = chain();
    for (record, text) in records.iter_mut().zip([first, second]) {
        record.created_at = CommonTimestampV1::new(text.to_owned()).unwrap();
    }
    records
}

#[test]
fn rejects_a_creation_instant_that_goes_back() {
    for (first, second) in [
        ("2030-01-02T03:04:05Z", "2030-01-02T03:04:04Z"),
        ("2030-01-02T00:00:00Z", "2030-01-01T23:59:59.999999Z"),
        ("2030-01-02T03:04:05.000001Z", "2030-01-02T03:04:05Z"),
        // The second instant is earlier, although it sorts later as text.
        ("2030-01-02T03:04:05.5Z", "2030-01-02T03:04:05Z"),
    ] {
        assert_eq!(
            validate_chain(&chain_created_at(first, second)),
            Err(ChainError::CreatedAt),
            "{first} then {second}"
        );
    }
}

#[test]
fn accepts_equal_creation_instants_however_they_are_written() {
    for (first, second) in [
        ("2030-01-02T03:04:05Z", "2030-01-02T03:04:05Z"),
        ("2030-01-02T03:04:05Z", "2030-01-02T03:04:05.000000Z"),
        ("2030-01-02T03:04:05.5Z", "2030-01-02T03:04:05.500Z"),
        ("2030-01-02T03:04:05.500Z", "2030-01-02T03:04:05.5Z"),
    ] {
        assert_eq!(
            validate_chain(&chain_created_at(first, second)),
            Ok(()),
            "{first} then {second}"
        );
    }
}

#[test]
fn orders_creation_instants_by_time_and_not_by_text() {
    for (first, second) in [
        // The second instant is later, although it sorts earlier as text.
        ("2030-01-02T03:04:05Z", "2030-01-02T03:04:05.5Z"),
        ("2030-01-02T03:04:05.5Z", "2030-01-02T03:04:05.50001Z"),
        // A leap second comes before the first instant of the next day.
        ("2030-12-31T23:59:60.5Z", "2031-01-01T00:00:00.2Z"),
    ] {
        assert_eq!(
            validate_chain(&chain_created_at(first, second)),
            Ok(()),
            "{first} then {second}"
        );
    }
}

#[test]
fn rejects_a_repeated_state_document() {
    let mut records = chain();
    records[1].state_document_id = records[0].state_document_id.clone();
    assert_eq!(
        validate_chain(&records),
        Err(ChainError::RepeatedStateDocument)
    );
}

#[test]
fn rejects_a_repeated_opening_prompt() {
    let mut records = chain();
    records[1].opening_prompt_id = records[0].opening_prompt_id.clone();
    assert_eq!(
        validate_chain(&records),
        Err(ChainError::RepeatedOpeningPrompt)
    );
}

const OWNER: &str = "user-1";

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text.to_owned()).unwrap()
}

/// device-1 belongs to the owner; device-2 to someone else; device-3 was revoked at 2030.
fn registry(device: &CommonIdV1, at: &CommonTimestampV1) -> Option<CommonIdV1> {
    match device.as_str() {
        "device-1" => Some(id(OWNER)),
        "device-2" => Some(id("user-2")),
        "device-3" if at.as_str() < "2030-01-01T00:00:00Z" => Some(id(OWNER)),
        _ => None,
    }
}

/// Accepted by device-1 for handoff-1 and session-1, the event the first golden record names.
fn accept_event(change: impl FnOnce(&mut serde_json::Value)) -> EventSessionHandoffV4 {
    let mut event: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/golden/event.session.handoff.v4/valid/02.json"
    ))
    .unwrap();
    change(&mut event);
    serde_json::from_value(event).unwrap()
}

fn verify(
    record: &RecordSessionHandoffV2,
    event: &EventSessionHandoffV4,
) -> Result<(), ConsentError> {
    verify_consent(record, &id(OWNER), event, registry)
}

#[test]
fn confirms_the_acceptance_the_record_refers_to() {
    let [mut record, _] = chain();
    for trigger in [Trigger::ContextLimit, Trigger::AccountSwitch] {
        record.trigger = trigger;
        for channel in [Channel::Local, Channel::Remote] {
            record.consent.channel = channel;
            let event = accept_event(|event| {
                event["data"]["trigger"] = json!(trigger);
                event["data"]["channel"] = json!(channel);
            });
            assert_eq!(verify(&record, &event), Ok(()), "{trigger:?} {channel:?}");
        }
    }
}

#[test]
fn rejects_consent_for_another_trigger() {
    let [record, _] = chain();
    let event = accept_event(|event| event["data"]["trigger"] = json!("account_switch"));
    assert_eq!(verify(&record, &event), Err(ConsentError::OtherTrigger));
}

#[test]
fn rejects_consent_on_another_channel() {
    let [record, _] = chain();
    let event = accept_event(|event| event["data"]["channel"] = json!("remote"));
    assert_eq!(verify(&record, &event), Err(ConsentError::OtherChannel));
}

#[test]
fn historical_acceptance_is_readable_only_as_its_own_version() {
    let text = include_str!("../../../contracts/golden/event.session.handoff.v3/valid/02.json");
    assert!(serde_json::from_str::<EventSessionHandoffV3>(text).is_ok());
    assert!(serde_json::from_str::<EventSessionHandoffV4>(text).is_err());
    let mut event: serde_json::Value = serde_json::from_str(text).unwrap();
    event["data"]["trigger"] = json!("context_limit");
    event["data"]["channel"] = json!("local");
    assert!(serde_json::from_value::<EventSessionHandoffV4>(event).is_err());
}

#[test]
fn a_model_refusal_is_not_confirmed_without_its_offer() {
    let [mut record, _] = chain();
    record.trigger = Trigger::ModelRefusal;
    for event in [
        accept_event(|_| {}),
        accept_event(|event| {
            event["data"]["trigger"] = json!("model_refusal");
            event["data"]["alternative_id"] = json!("b-z");
        }),
    ] {
        assert_eq!(verify(&record, &event), Err(ConsentError::ModelRefusal));
    }
}

#[test]
fn an_agent_or_the_daemon_cannot_confirm() {
    let [record, _] = chain();
    // The registry names the owner for any id, so only the actor kind can refuse.
    let anyone = |_: &CommonIdV1, _: &CommonTimestampV1| Some(id(OWNER));
    // The contract admits an agent or the daemon only on a proposal.
    for kind in ["agent", "daemon"] {
        let event = accept_event(|event| {
            event["data"]["status"] = json!("proposed");
            event["data"].as_object_mut().unwrap().remove("channel");
            event["actor"]["kind"] = json!(kind);
        });
        assert_eq!(
            verify_consent(&record, &id(OWNER), &event, anyone),
            Err(ConsentError::NotDevice),
            "{kind}"
        );
    }
}

#[test]
fn only_a_device_registered_to_the_owner_confirms() {
    let [record, _] = chain();
    for device in ["device-2", "device-3", "device-9"] {
        let event = accept_event(|event| event["actor"]["id"] = json!(device));
        assert_eq!(
            verify(&record, &event),
            Err(ConsentError::NotOwnerDevice),
            "{device}"
        );
    }
}

#[test]
fn the_registry_is_read_at_the_instant_of_the_event() {
    let [record, _] = chain();
    let before = accept_event(|event| {
        event["actor"]["id"] = json!("device-3");
        event["ts"] = json!("2029-12-31T23:59:59Z");
    });
    assert_eq!(verify(&record, &before), Ok(()));
}

#[test]
fn the_owner_comes_from_the_caller_and_not_from_the_record() {
    let [mut record, _] = chain();
    // The record and the device agree on another user; the lineage owner is still user-1.
    record.consent.user_id = id("user-2");
    let event = accept_event(|event| event["actor"]["id"] = json!("device-2"));
    assert_eq!(verify(&record, &event), Err(ConsentError::NotOwnerDevice));
}

#[test]
fn only_an_acceptance_confirms() {
    let [record, _] = chain();
    for status in ["proposed", "declined"] {
        let event = accept_event(|event| {
            event["data"]["status"] = json!(status);
            if status == "proposed" {
                event["data"].as_object_mut().unwrap().remove("channel");
            }
        });
        assert_eq!(
            verify(&record, &event),
            Err(ConsentError::NotAccepted),
            "{status}"
        );
    }
}

#[test]
fn rejects_an_acceptance_of_another_handoff() {
    let [record, _] = chain();
    let event = accept_event(|event| event["data"]["handoff_id"] = json!("handoff-9"));
    assert_eq!(verify(&record, &event), Err(ConsentError::OtherHandoff));
}

#[test]
fn rejects_an_acceptance_for_another_source_session() {
    let [record, _] = chain();
    let event = accept_event(|event| event["data"]["source_session_id"] = json!("session-9"));
    assert_eq!(verify(&record, &event), Err(ConsentError::OtherSession));
}

#[test]
fn rejects_an_event_the_consent_does_not_refer_to() {
    let [mut record, _] = chain();
    record.consent.event_id = CommonUlidV1::new("01ARZ3NDEKTSV4RRFFQ69G5FAD".to_owned()).unwrap();
    assert_eq!(
        verify(&record, &accept_event(|_| {})),
        Err(ConsentError::OtherEvent)
    );
}

#[test]
fn rejects_a_consent_that_names_another_user() {
    let [mut record, _] = chain();
    record.consent.user_id = id("user-2");
    assert_eq!(
        verify(&record, &accept_event(|_| {})),
        Err(ConsentError::OtherUser)
    );
}
