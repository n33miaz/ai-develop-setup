use std::num::NonZeroU64;

use ads_contracts::{
    CommonIdV1, RecordSessionHandoffV2, RecordSessionHandoffV2Number,
    RecordSessionHandoffV2Trigger as Trigger,
};
use ads_core::handoff::{
    ChainError, Consent, ContextUsage, DEFAULT_POLICY, Decision, HandoffPolicy, next_session_name,
    validate_chain,
};

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
