use ads_contracts::{
    CommonDataClassV1 as Class, CommonIdV1, EventSessionHandoffV2, EventSessionRefusalDetectedV1,
    EventSessionRefusalDetectedV1Data, RecordSessionHandoffV2Trigger as Trigger,
    RecordTaskPacketV1,
};
use ads_core::budget::{BudgetDecision, Dimension};
use ads_core::handoff::HandoffPolicy;
use ads_core::refusal::{AcceptError, DEFAULT_POLICY, Offer, RefusalPolicy, RouteContext};
use ads_core::routing::{
    Candidate, QuotaObservation, QuotaState, Route, RoutingPolicy, Size, TaskProfile,
};
use serde_json::json;

const TASK: &str =
    include_str!("../../../contracts/golden/record.task-packet.v1/valid/01-base.json");
const REFUSAL: &str =
    include_str!("../../../contracts/golden/event.session.refusal_detected.v1/valid/01.json");
const ACCEPTED: &str =
    include_str!("../../../contracts/golden/event.session.handoff.v2/valid/02.json");
const PROPOSED: &str =
    include_str!("../../../contracts/golden/event.session.handoff.v2/valid/01.json");

// A single rule keeps every tier eligible, so the tests see the order of the catalog.
const ROUTING: &str = r#"
mode = "shadow"
quota_max_age_secs = 900
quota_block_at_pct = 100.0
allow_unknown_quota = true

[[rules]]
id = "all"
tiers = ["T0", "T1", "T2", "T3"]
"#;

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn candidate(name: &str, harness: &str, model: &str, rank: u32) -> Candidate {
    serde_json::from_value(json!({
        "id": name,
        "rank": rank,
        "allowed_sensitivity": ["internal"],
        "route": {
            "harness": harness, "provider": format!("{harness}-provider"),
            "model": model, "tier": "T1", "effort": "medium"
        }
    }))
    .unwrap()
}

/// The refused route is harness-a/model-x, as in the golden refusal.
fn catalog() -> Vec<Candidate> {
    vec![
        candidate("a-x", "harness-a", "model-x", 0),
        candidate("a-y", "harness-a", "model-y", 1),
        candidate("b-z", "harness-b", "model-z", 2),
        candidate("c-w", "harness-c", "model-w", 3),
    ]
}

fn refusal(harness: &str, model: &str) -> EventSessionRefusalDetectedV1Data {
    let mut event: serde_json::Value = serde_json::from_str(REFUSAL).unwrap();
    event["data"]["harness"] = json!(harness);
    event["data"]["model"] = json!(model);
    serde_json::from_value::<EventSessionRefusalDetectedV1>(event)
        .unwrap()
        .data
}

fn handoff(mode: &str) -> HandoffPolicy {
    HandoffPolicy::from_toml(&format!("mode = \"{mode}\"\ncontext_threshold_pct = 80")).unwrap()
}

fn policy(text: &str) -> RefusalPolicy {
    RefusalPolicy::from_toml(text).unwrap()
}

fn settings(enabled: bool, scope: &str, order: &str, max: u8) -> RefusalPolicy {
    policy(&format!(
        "enabled = {enabled}\nscope = \"{scope}\"\norder = \"{order}\"\nmax_alternatives = {max}"
    ))
}

struct Fixture {
    routing: RoutingPolicy,
    task: RecordTaskPacketV1,
    kind: CommonIdV1,
    budget: BudgetDecision,
    quota: Vec<QuotaObservation>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            routing: toml::from_str(ROUTING).unwrap(),
            task: serde_json::from_str(TASK).unwrap(),
            kind: id("code"),
            budget: BudgetDecision::Continue,
            quota: Vec::new(),
        }
    }

    fn offer(
        &self,
        refusal: &RefusalPolicy,
        handoff: &HandoffPolicy,
        refusals: &[EventSessionRefusalDetectedV1Data],
        candidates: &[Candidate],
    ) -> Option<Offer> {
        let profile = TaskProfile {
            kind: &self.kind,
            size: Size::Medium,
            sensitivity: Class::Internal,
        };
        refusal.offer(
            handoff,
            id("handoff-1"),
            refusals,
            &RouteContext {
                routing: &self.routing,
                task: &self.task,
                profile: &profile,
                candidates,
                quota: &self.quota,
                now_secs: 1_000,
                budget: &self.budget,
            },
        )
    }
}

fn models(offer: &Offer) -> Vec<&str> {
    offer
        .alternatives
        .iter()
        .map(|route| route.model.as_str())
        .collect()
}

#[test]
fn default_policy_offers_other_tools_first() {
    let fixture = Fixture::new();
    let offer = fixture
        .offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &[refusal("harness-a", "model-x")],
            &catalog(),
        )
        .unwrap();
    assert_eq!(models(&offer), ["model-z", "model-w", "model-y"]);
    assert_eq!(offer.trigger, Trigger::ModelRefusal);
    assert_eq!(offer.source_session_id, id("session-1"));
    assert_eq!(offer.handoff_id, id("handoff-1"));
}

#[test]
fn never_offers_a_harness_and_model_that_refused() {
    let fixture = Fixture::new();
    let mut candidates = catalog();
    let mut variant = candidate("a-x-high", "harness-a", "model-x", 0);
    variant.route.provider = id("other-provider");
    variant.route.effort = None;
    candidates.push(variant);
    let refusal_policy = settings(true, "any-tool", "routing", 10);
    let offer = fixture
        .offer(
            &refusal_policy,
            &handoff("propose"),
            &[refusal("harness-a", "model-x")],
            &candidates,
        )
        .unwrap();
    assert_eq!(models(&offer), ["model-y", "model-z", "model-w"]);

    let offer = fixture
        .offer(
            &refusal_policy,
            &handoff("propose"),
            &[
                refusal("harness-a", "model-x"),
                refusal("harness-b", "model-z"),
            ],
            &candidates,
        )
        .unwrap();
    assert_eq!(models(&offer), ["model-y", "model-w"]);
}

#[test]
fn same_tool_scope_and_order_settings() {
    let fixture = Fixture::new();
    let refused = [refusal("harness-a", "model-x")];
    let same = fixture
        .offer(
            &settings(true, "same-tool", "routing", 10),
            &handoff("propose"),
            &refused,
            &catalog(),
        )
        .unwrap();
    assert_eq!(models(&same), ["model-y"]);

    let first = fixture
        .offer(
            &settings(true, "any-tool", "same-tool-first", 10),
            &handoff("propose"),
            &refused,
            &catalog(),
        )
        .unwrap();
    assert_eq!(models(&first), ["model-y", "model-z", "model-w"]);

    let limited = fixture
        .offer(
            &settings(true, "any-tool", "other-tool-first", 1),
            &handoff("propose"),
            &refused,
            &catalog(),
        )
        .unwrap();
    assert_eq!(models(&limited), ["model-z"]);
}

#[test]
fn order_does_not_depend_on_catalog_order() {
    let fixture = Fixture::new();
    let refused = [refusal("harness-a", "model-x")];
    let expected = fixture
        .offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &refused,
            &catalog(),
        )
        .unwrap();
    let mut candidates = catalog();
    candidates.reverse();
    candidates.swap(0, 2);
    for _ in 0..3 {
        assert_eq!(
            fixture.offer(
                &policy(DEFAULT_POLICY),
                &handoff("propose"),
                &refused,
                &candidates
            ),
            Some(expected.clone())
        );
    }
}

#[test]
fn disabled_settings_offer_nothing() {
    let fixture = Fixture::new();
    let refused = [refusal("harness-a", "model-x")];
    assert_eq!(
        fixture.offer(
            &settings(false, "any-tool", "routing", 3),
            &handoff("propose"),
            &refused,
            &catalog()
        ),
        None
    );
    assert_eq!(
        fixture.offer(
            &policy(DEFAULT_POLICY),
            &handoff("off"),
            &refused,
            &catalog()
        ),
        None
    );
    assert_eq!(
        fixture.offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &[],
            &catalog()
        ),
        None
    );
}

#[test]
fn respects_budget_quota_and_sensitivity() {
    let mut fixture = Fixture::new();
    let refused = [refusal("harness-a", "model-x")];
    fixture.budget = BudgetDecision::Stop(vec![Dimension::Usd]);
    assert_eq!(
        fixture.offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &refused,
            &catalog()
        ),
        None
    );

    fixture.budget = BudgetDecision::Warn(vec![Dimension::WallTime]);
    fixture.quota = vec![QuotaObservation {
        harness: id("harness-b"),
        provider: id("harness-b-provider"),
        observed_at_secs: 990,
        state: QuotaState::RateLimited,
    }];
    let mut candidates = catalog();
    candidates[3].allowed_sensitivity = vec![Class::Public];
    let offer = fixture
        .offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &refused,
            &candidates,
        )
        .unwrap();
    assert_eq!(models(&offer), ["model-y"]);
}

fn consent(text: &str, change: impl FnOnce(&mut serde_json::Value)) -> EventSessionHandoffV2 {
    let mut event: serde_json::Value = serde_json::from_str(text).unwrap();
    change(&mut event);
    serde_json::from_value(event).unwrap()
}

#[test]
fn proceeds_only_with_device_consent_for_an_offered_route() {
    let fixture = Fixture::new();
    let offer = fixture
        .offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &[refusal("harness-a", "model-x")],
            &catalog(),
        )
        .unwrap();
    let chosen = offer.alternatives[1].clone();
    let accepted = consent(ACCEPTED, |_| {});
    for mode in ["propose", "auto-with-consent"] {
        assert_eq!(
            offer.accept(&handoff(mode), &accepted, &chosen),
            Ok(chosen.clone())
        );
    }

    let refused: Route = catalog()[0].route.clone();
    assert_eq!(
        offer.accept(&handoff("propose"), &accepted, &refused),
        Err(AcceptError::NotOffered)
    );
    assert_eq!(
        offer.accept(&handoff("off"), &accepted, &chosen),
        Err(AcceptError::Policy)
    );
    assert_eq!(
        offer.accept(&handoff("propose"), &consent(PROPOSED, |_| {}), &chosen),
        Err(AcceptError::NotAccepted)
    );
    let declined = consent(ACCEPTED, |event| {
        event["data"]["status"] = json!("declined")
    });
    assert_eq!(
        offer.accept(&handoff("propose"), &declined, &chosen),
        Err(AcceptError::NotAccepted)
    );
    let other = consent(ACCEPTED, |event| {
        event["data"]["handoff_id"] = json!("handoff-2");
        event["stream"] = json!("handoff/handoff-2");
    });
    assert_eq!(
        offer.accept(&handoff("propose"), &other, &chosen),
        Err(AcceptError::OtherHandoff)
    );
    let elsewhere = consent(ACCEPTED, |event| {
        event["data"]["source_session_id"] = json!("session-9");
    });
    assert_eq!(
        offer.accept(&handoff("propose"), &elsewhere, &chosen),
        Err(AcceptError::OtherSession)
    );
}

#[test]
fn rejects_invalid_settings() {
    for text in [
        "enabled = true\nscope = \"any-tool\"\norder = \"routing\"\nmax_alternatives = 0",
        "enabled = true\nscope = \"any-tool\"\norder = \"routing\"\nmax_alternatives = 11",
        "enabled = true\nscope = \"anywhere\"\norder = \"routing\"\nmax_alternatives = 3",
        "enabled = true\nscope = \"any-tool\"\norder = \"random\"\nmax_alternatives = 3",
        "enabled = true\nscope = \"any-tool\"\norder = \"routing\"\nmax_alternatives = 3\nauto = true",
    ] {
        assert!(RefusalPolicy::from_toml(text).is_err(), "{text}");
    }
}
