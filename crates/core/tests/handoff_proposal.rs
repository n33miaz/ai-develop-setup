use std::num::NonZeroU64;

use ads_contracts::{
    CommonIdV1, EventSessionHandoffProposedV2, EventSessionHandoffProposedV2Data,
    EventSessionHandoffProposedV2DataNumber as Number,
};
use ads_core::handoff::{ContextUsage, DEFAULT_POLICY, HandoffPolicy};
use ads_core::handoff_proposal::{ProposalError, proposed};
use serde_json::Value;

const URN: &str = "urn:ads:event:session.handoff_proposed:2";
const FIRST: &str =
    include_str!("../../../contracts/golden/event.session.handoff_proposed.v2/valid/01.json");
const SECOND: &str = include_str!(
    "../../../contracts/golden/event.session.handoff_proposed.v2/valid/03-second-with-predecessor.json"
);

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text.to_owned()).unwrap()
}

fn policy(mode: &str, pct: u8) -> HandoffPolicy {
    HandoffPolicy::from_toml(&format!("mode = \"{mode}\"\ncontext_threshold_pct = {pct}")).unwrap()
}

fn usage(used: u64, capacity: u64) -> Option<ContextUsage> {
    Some(ContextUsage {
        used,
        capacity: NonZeroU64::new(capacity).unwrap(),
    })
}

fn first(
    policy: &HandoffPolicy,
    usage: Option<ContextUsage>,
) -> Option<EventSessionHandoffProposedV2Data> {
    proposed(
        policy,
        id("session-1"),
        usage,
        Number::new(1).unwrap(),
        None,
    )
    .unwrap()
}

fn percentages(data: &EventSessionHandoffProposedV2Data) -> (u16, u16) {
    (data.threshold_pct.get(), data.context_pct.get())
}

#[test]
fn proposes_from_the_threshold_on() {
    let default = HandoffPolicy::from_toml(DEFAULT_POLICY).unwrap();
    assert_eq!(first(&default, usage(79, 100)), None);
    assert_eq!(first(&default, usage(799, 1000)), None);
    let data = first(&default, usage(80, 100)).unwrap();
    assert_eq!(percentages(&data), (80, 80));
}

#[test]
fn reports_the_configured_threshold_in_both_active_modes() {
    for mode in ["propose", "auto-with-consent"] {
        for threshold in [1, 50, 100] {
            let policy = policy(mode, threshold);
            let below = u64::from(threshold) * 10 - 1;
            assert_eq!(first(&policy, usage(below, 1000)), None);
            let data = first(&policy, usage(u64::from(threshold), 100)).unwrap();
            let threshold = u16::from(threshold);
            assert_eq!(percentages(&data), (threshold, threshold), "{mode}");
        }
    }
}

#[test]
fn rounds_the_measured_percentage_down() {
    let data = first(&policy("propose", 80), usage(839, 1000)).unwrap();
    assert_eq!(percentages(&data), (80, 83));
}

#[test]
fn limits_usage_above_the_capacity_to_the_whole_window() {
    for usage in [usage(101, 100), usage(150, 100), usage(u64::MAX, 1)] {
        let data = first(&policy("propose", 80), usage).unwrap();
        assert_eq!(percentages(&data), (80, 100));
    }
}

#[test]
fn proposes_nothing_without_a_measurement() {
    assert_eq!(first(&policy("propose", 1), None), None);
}

#[test]
fn proposes_nothing_when_off() {
    assert_eq!(first(&policy("off", 80), usage(100, 100)), None);
}

#[test]
fn rejects_a_predecessor_of_the_first_session() {
    for used in [10, 90] {
        let result = proposed(
            &policy("propose", 80),
            id("session-1"),
            usage(used, 100),
            Number::new(1).unwrap(),
            Some(id("document-1")),
        );
        assert!(
            matches!(result, Err(ProposalError::FirstWithPredecessor)),
            "{result:?}"
        );
    }
}

#[test]
fn rejects_a_later_session_without_predecessor() {
    for (used, number) in [(10, 2), (90, 2), (90, 9007199254740991)] {
        let result = proposed(
            &policy("propose", 80),
            id("session-2"),
            usage(used, 100),
            Number::new(number).unwrap(),
            None,
        );
        assert!(
            matches!(result, Err(ProposalError::MissingPredecessor)),
            "{result:?}"
        );
    }
}

#[test]
fn built_events_match_the_contract() {
    for (golden, session, number, previous) in [
        (FIRST, "session-1", 1, None),
        (SECOND, "session-2", 2, Some("document-1")),
    ] {
        let data = proposed(
            &policy("propose", 80),
            id(session),
            usage(839, 1000),
            Number::new(number).unwrap(),
            previous.map(id),
        )
        .unwrap()
        .unwrap();
        let expected: Value = serde_json::from_str(golden).unwrap();
        let mut event = expected.clone();
        event["data"] = serde_json::to_value(data).unwrap();
        assert_eq!(event, expected);
        serde_json::from_value::<EventSessionHandoffProposedV2>(event.clone()).unwrap();
        ads_contracts::reencode(URN, event).unwrap().unwrap();
    }
}
