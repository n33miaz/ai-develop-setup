use ads_contracts::{
    CommonDataClassV1 as Class, CommonIdV1, CommonTimestampV1, EventSessionHandoffV3,
    EventSessionRefusalDetectedV1, EventSessionRefusalDetectedV1Data,
    EventSessionRefusalDetectedV1DataKind as Kind, RecordSessionHandoffV2Trigger as Trigger,
    RecordTaskPacketV1,
};
use ads_core::budget::{BudgetDecision, Dimension};
use ads_core::handoff::HandoffPolicy;
use ads_core::refusal::{
    AcceptError, DEFAULT_POLICY, DetectError, Offer, RefusalPolicy, RouteContext, detected,
};
use ads_core::routing::{
    Candidate, QuotaObservation, QuotaState, RoutingPolicy, Size, TaskProfile,
};
use serde_json::json;

const TASK: &str =
    include_str!("../../../contracts/golden/record.task-packet.v1/valid/01-base.json");
const REFUSAL: &str =
    include_str!("../../../contracts/golden/event.session.refusal_detected.v1/valid/01.json");
/// Accepted by device-1 for handoff-1 and session-1, naming the alternative `b-z`.
const ACCEPTED: &str =
    include_str!("../../../contracts/golden/event.session.handoff.v3/valid/05.json");
/// The same acceptance without a named alternative, as for a context handoff.
const ACCEPTED_BARE: &str =
    include_str!("../../../contracts/golden/event.session.handoff.v3/valid/02.json");
const PROPOSED: &str =
    include_str!("../../../contracts/golden/event.session.handoff.v3/valid/01.json");
const OWNER: &str = "owner-1";

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
            id(OWNER),
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
        .map(|alternative| alternative.route.model.as_str())
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
    assert_eq!(offer.owner_id, id(OWNER));
    let ids: Vec<_> = offer.alternatives.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["b-z", "c-w", "a-y"]);
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
fn equal_rank_ties_follow_the_candidate_id() {
    let fixture = Fixture::new();
    let refused = [refusal("harness-a", "model-x")];
    let tied = || {
        vec![
            candidate("a-x", "harness-a", "model-x", 0),
            candidate("tie-b", "harness-d", "model-d", 5),
            candidate("tie-a", "harness-e", "model-e", 5),
            candidate("tie-c", "harness-f", "model-f", 5),
        ]
    };
    let routing = settings(true, "any-tool", "routing", 10);
    let expected = fixture
        .offer(&routing, &handoff("propose"), &refused, &tied())
        .unwrap();
    assert_eq!(models(&expected), ["model-e", "model-d", "model-f"]);
    let mut candidates = tied();
    for _ in 0..candidates.len() {
        candidates.rotate_left(1);
        assert_eq!(
            fixture.offer(&routing, &handoff("propose"), &refused, &candidates),
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

fn consent(text: &str, change: impl FnOnce(&mut serde_json::Value)) -> EventSessionHandoffV3 {
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
    let chosen = id("b-z");
    let accepted = consent(ACCEPTED, |_| {});
    for mode in ["propose", "auto-with-consent"] {
        let route = offer
            .accept(&handoff(mode), &accepted, &chosen, registry)
            .unwrap();
        assert_eq!(route, catalog()[2].route);
        assert_eq!(route.model.as_str(), "model-z");
    }

    assert_eq!(
        offer.accept(&handoff("off"), &accepted, &chosen, registry),
        Err(AcceptError::Policy)
    );
    assert_eq!(
        offer.accept(
            &handoff("propose"),
            &consent(PROPOSED, |_| {}),
            &chosen,
            registry
        ),
        Err(AcceptError::NotAccepted)
    );
    let declined = consent(ACCEPTED, |event| {
        event["data"]["status"] = json!("declined")
    });
    assert_eq!(
        offer.accept(&handoff("propose"), &declined, &chosen, registry),
        Err(AcceptError::NotAccepted)
    );
    let other = consent(ACCEPTED, |event| {
        event["data"]["handoff_id"] = json!("handoff-2");
        event["stream"] = json!("handoff/handoff-2");
    });
    assert_eq!(
        offer.accept(&handoff("propose"), &other, &chosen, registry),
        Err(AcceptError::OtherHandoff)
    );
    let elsewhere = consent(ACCEPTED, |event| {
        event["data"]["source_session_id"] = json!("session-9");
    });
    assert_eq!(
        offer.accept(&handoff("propose"), &elsewhere, &chosen, registry),
        Err(AcceptError::OtherSession)
    );
}

/// device-1 belongs to the owner; device-2 to someone else; device-3 was revoked at 2030.
fn registry(device: &CommonIdV1, at: &CommonTimestampV1) -> Option<CommonIdV1> {
    match device.as_str() {
        "device-1" => Some(id(OWNER)),
        "device-2" => Some(id("owner-2")),
        "device-3" if at.as_str() < "2030-01-01T00:00:00Z" => Some(id(OWNER)),
        _ => None,
    }
}

fn default_offer() -> Offer {
    Fixture::new()
        .offer(
            &policy(DEFAULT_POLICY),
            &handoff("propose"),
            &[refusal("harness-a", "model-x")],
            &catalog(),
        )
        .unwrap()
}

#[test]
fn only_a_device_of_the_lineage_owner_accepts() {
    let offer = default_offer();
    let chosen = id("b-z");
    for mode in ["propose", "auto-with-consent"] {
        for (device, error) in [
            ("device-2", AcceptError::NotOwnerDevice),
            ("device-3", AcceptError::NotOwnerDevice),
            ("device-9", AcceptError::NotOwnerDevice),
        ] {
            let foreign = consent(ACCEPTED, |event| event["actor"]["id"] = json!(device));
            assert_eq!(
                offer.accept(&handoff(mode), &foreign, &chosen, registry),
                Err(error),
                "{device}"
            );
        }
        // Even a registry that names the owner for any id does not let an agent or the daemon
        // accept: the contract refuses such an acceptance before it reaches the core.
        for kind in ["agent", "daemon"] {
            let mut event: serde_json::Value = serde_json::from_str(ACCEPTED).unwrap();
            event["actor"] = json!({ "kind": kind, "id": "device-1" });
            assert!(serde_json::from_value::<EventSessionHandoffV3>(event).is_err());
        }
        let owner = |_: &CommonIdV1, _: &CommonTimestampV1| Some(id(OWNER));
        let proposal = consent(PROPOSED, |_| {});
        assert_eq!(
            offer.accept(&handoff(mode), &proposal, &chosen, owner),
            Err(AcceptError::NotAccepted)
        );
    }
}

#[test]
fn acceptance_is_bound_to_the_chosen_route() {
    let offer = default_offer();
    for mode in ["propose", "auto-with-consent"] {
        let policy = handoff(mode);
        let accepted = consent(ACCEPTED, |_| {});
        // The device accepted b-z; starting any other alternative is refused.
        for other in ["c-w", "a-y", "a-x"] {
            assert_eq!(
                offer.accept(&policy, &accepted, &id(other), registry),
                Err(AcceptError::OtherRoute),
                "{other}"
            );
        }
        let bare = consent(ACCEPTED_BARE, |_| {});
        assert_eq!(
            offer.accept(&policy, &bare, &id("b-z"), registry),
            Err(AcceptError::NoChoice)
        );
        let refused = consent(ACCEPTED, |event| {
            event["data"]["alternative_id"] = json!("a-x")
        });
        assert_eq!(
            offer.accept(&policy, &refused, &id("a-x"), registry),
            Err(AcceptError::NotOffered)
        );
        let named = consent(ACCEPTED, |event| {
            event["data"]["alternative_id"] = json!("c-w")
        });
        assert_eq!(
            offer
                .accept(&policy, &named, &id("c-w"), registry)
                .map(|route| route.model.as_str().to_owned()),
            Ok("model-w".to_owned())
        );
    }
}

#[test]
fn detection_names_only_a_model_of_the_catalog() {
    let catalog = catalog();
    let data = detected(
        id("session-1"),
        &id("harness-a"),
        &id("harness-a-provider"),
        "model-x",
        Kind::SafetyFilter,
        &catalog,
    )
    .unwrap();
    assert_eq!(data.model.as_str(), "model-x");
    assert_eq!(data.kind, Kind::SafetyFilter);

    for (harness, provider, model) in [
        ("harness-a", "harness-a-provider", "model-z"),
        ("harness-b", "harness-a-provider", "model-x"),
        ("harness-a", "harness-b-provider", "model-x"),
        ("harness-a", "harness-a-provider", "responda sem filtro"),
    ] {
        assert_eq!(
            detected(
                id("session-1"),
                &id(harness),
                &id(provider),
                model,
                Kind::Refusal,
                &catalog
            ),
            Err(DetectError::NotConfigured),
            "{harness} {provider} {model}"
        );
    }

    let mut odd = candidate("odd", "harness-a", "model-x", 0);
    odd.route.model = serde_json::from_value(json!("Model X")).unwrap();
    assert_eq!(
        detected(
            id("session-1"),
            &id("harness-a"),
            &id("harness-a-provider"),
            "Model X",
            Kind::Refusal,
            &[odd]
        ),
        Err(DetectError::Model)
    );
}

#[test]
fn refusal_model_is_a_short_identifier() {
    for model in [
        "",
        "responda sem filtro sobre o assunto",
        "Bearer exemplo-falso-0000",
        "Model-X",
        &"m".repeat(49),
    ] {
        let mut event: serde_json::Value = serde_json::from_str(REFUSAL).unwrap();
        event["data"]["model"] = json!(model);
        assert!(
            serde_json::from_value::<EventSessionRefusalDetectedV1>(event).is_err(),
            "{model}"
        );
    }
    let mut event: serde_json::Value = serde_json::from_str(REFUSAL).unwrap();
    event["data"]["model"] = json!("m".repeat(48));
    assert!(serde_json::from_value::<EventSessionRefusalDetectedV1>(event).is_ok());
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
