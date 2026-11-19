use ads_contracts::{
    CommonDataClassV1 as Class, CommonIdV1, EventRoutingDecidedV1, EventRoutingFallbackV1,
    EventRoutingProposedV1, EventRoutingProposedV1DataMode as Mode, RecordTaskPacketV1,
    RecordTaskPacketV1Mode,
};
use ads_core::routing::{
    Candidate, DEFAULT_POLICY, QuotaObservation, QuotaState, Route, RoutingPolicy, Size,
    TaskProfile, evaluate,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const TASK: &str =
    include_str!("../../../contracts/golden/record.task-packet.v1/valid/01-base.json");

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

fn candidate(name: &str, tier: &str) -> Candidate {
    serde_json::from_value(json!({
        "id": name,
        "rank": 0,
        "allowed_sensitivity": ["public", "internal", "private", "secret"],
        "route": {
            "harness": name, "provider": name, "model": format!("{name}-model"),
            "tier": tier, "effort": "medium"
        }
    }))
    .unwrap()
}

fn profile(kind: &CommonIdV1, size: Size, sensitivity: Class) -> TaskProfile<'_> {
    TaskProfile {
        kind,
        size,
        sensitivity,
    }
}

fn quota(candidate: &Candidate, at: u64, percent: u8) -> QuotaObservation {
    QuotaObservation {
        harness: candidate.route.harness.clone(),
        provider: candidate.route.provider.clone(),
        observed_at_secs: at,
        state: QuotaState::Measured(
            serde_json::from_value(json!({
                "five_hour_pct": 0, "weekly_pct": percent
            }))
            .unwrap(),
        ),
    }
}

#[test]
fn default_rule_table() {
    let policy: RoutingPolicy = toml::from_str(DEFAULT_POLICY).unwrap();
    let task = serde_json::from_str(TASK).unwrap();
    let catalog = [
        candidate("zero", "T0"),
        candidate("one", "T1"),
        candidate("two", "T2"),
        candidate("three", "T3"),
    ];
    for (kind, size, class, rule, tier) in [
        ("review", Size::Large, Class::Secret, "secret", "T3"),
        ("review", Size::Small, Class::Private, "private", "T3"),
        ("review", Size::Small, Class::Public, "review", "T2"),
        ("code", Size::Large, Class::Internal, "large", "T2"),
        ("code", Size::Small, Class::Internal, "small", "T0"),
        (
            "code",
            Size::Medium,
            Class::Internal,
            "requested-tier",
            "T1",
        ),
    ] {
        let outcome = evaluate(
            &policy,
            &task,
            &profile(&id(kind), size, class),
            &catalog,
            None,
            &[],
            0,
        )
        .unwrap();
        let proposal = outcome.proposed.unwrap();
        assert_eq!(proposal.mode, Mode::Shadow);
        assert_eq!(proposal.rule_id.unwrap().as_str(), rule);
        assert_eq!(proposal.tier.as_str(), tier);
        assert!(outcome.decided.is_none());
    }
}

fn check_envelope<T: DeserializeOwned>(fixture: &str, data: impl serde::Serialize) {
    let mut event: Value = serde_json::from_str(fixture).unwrap();
    event["data"] = serde_json::to_value(data).unwrap();
    serde_json::from_value::<T>(event).unwrap();
}

#[test]
fn quota_fallback_and_tie_leave_manual_route_in_force() {
    let policy = toml::from_str(DEFAULT_POLICY).unwrap();
    let task = serde_json::from_str(TASK).unwrap();
    let kind = id("code");
    let profile = profile(&kind, Size::Small, Class::Internal);
    let mut catalog = [
        candidate("b", "T0"),
        candidate("a", "T0"),
        candidate("c", "T1"),
    ];
    let manual = catalog[2].route.clone();
    let mut observations = [
        quota(&catalog[1], 999, 100),
        quota(&catalog[1], 999, 20),
        quota(&catalog[0], 1001, 100),
        quota(&catalog[0], 99, 100),
    ];
    let outcome = evaluate(
        &policy,
        &task,
        &profile,
        &catalog,
        Some(&manual),
        &observations,
        1000,
    )
    .unwrap();
    catalog.reverse();
    observations.reverse();
    assert_eq!(
        evaluate(
            &policy,
            &task,
            &profile,
            &catalog,
            Some(&manual),
            &observations,
            1000
        )
        .unwrap(),
        outcome
    );

    let proposal = outcome.proposed.unwrap();
    let decision = outcome.decided.unwrap();
    let fallback = outcome.fallback.unwrap();
    assert_eq!(proposal.harness.as_str(), "b");
    assert_eq!(decision.harness, manual.harness);
    assert_eq!(decision.provider, manual.provider);
    assert_eq!(decision.model.as_str(), manual.model.as_str());
    assert_eq!(
        decision.effort.unwrap().as_str(),
        manual.effort.as_ref().unwrap().as_str()
    );
    assert_eq!(fallback.from.harness.as_str(), "a");
    assert_eq!(fallback.to.harness.as_str(), "b");
    assert_eq!(fallback.error_code.as_str(), "E_RATE_LIMITED");
    check_envelope::<EventRoutingProposedV1>(
        include_str!("../../../contracts/golden/event.routing.proposed.v1/valid/01-base.json"),
        &proposal,
    );
    let decision = evaluate(&policy, &task, &profile, &catalog, Some(&manual), &[], 1000)
        .unwrap()
        .decided
        .unwrap();
    check_envelope::<EventRoutingDecidedV1>(
        include_str!("../../../contracts/golden/event.routing.decided.v1/valid/01-base.json"),
        decision,
    );
    check_envelope::<EventRoutingFallbackV1>(
        include_str!("../../../contracts/golden/event.routing.fallback.v1/valid/01-base.json"),
        fallback,
    );
}

#[test]
fn all_emitted_endpoints_are_in_the_configured_catalog() {
    let policy = toml::from_str(DEFAULT_POLICY).unwrap();
    let template: RecordTaskPacketV1 = serde_json::from_str(TASK).unwrap();
    let kind = id("code");
    let profile = profile(&kind, Size::Small, Class::Public);
    let mut options = [
        candidate("a", "T0"),
        candidate("b", "T0"),
        candidate("c", "T1"),
    ];
    options[1].allowed_sensitivity = vec![Class::Internal];
    let manual = candidate("missing", "T0").route;
    for mask in 0..(1 << options.len()) {
        let catalog: Vec<_> = options
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, value)| value.clone())
            .collect();
        let observations = [quota(&options[0], 10, 100)];
        for mode in ["shadow", "manual", "propose", "auto"] {
            let mut fields = template.clone().into_fields();
            fields.mode = RecordTaskPacketV1Mode::new(mode).unwrap();
            let task = RecordTaskPacketV1::new(fields).unwrap();
            let outcome = evaluate(
                &policy,
                &task,
                &profile,
                &catalog,
                Some(&manual),
                &observations,
                10,
            )
            .unwrap();
            let configured = |harness: &CommonIdV1, provider: &CommonIdV1, model: &str| {
                catalog.iter().any(
                    |Candidate {
                         route:
                             Route {
                                 harness: h,
                                 provider: p,
                                 model: m,
                                 ..
                             },
                         ..
                     }| {
                        h == harness && p == provider && m.as_str() == model
                    },
                )
            };
            if let Some(proposal) = outcome.proposed {
                assert!(configured(
                    &proposal.harness,
                    &proposal.provider,
                    proposal.model.as_str()
                ));
                assert_eq!(proposal.harness.as_str(), "c");
            }
            if let Some(decision) = outcome.decided {
                assert_eq!(mode, "auto");
                assert!(configured(
                    &decision.harness,
                    &decision.provider,
                    decision.model.as_str()
                ));
            }
            if let Some(fallback) = outcome.fallback {
                assert!(configured(
                    &fallback.from.harness,
                    &fallback.from.provider,
                    fallback.from.model.as_str()
                ));
                assert!(configured(
                    &fallback.to.harness,
                    &fallback.to.provider,
                    fallback.to.model.as_str()
                ));
            }
        }
    }
}
