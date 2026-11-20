//! Offer other configured routes after a refusal or a safety filter.

use ads_contracts::{
    CommonActorV1Kind, CommonIdV1, EventSessionHandoffV2, EventSessionHandoffV2DataStatus,
    EventSessionRefusalDetectedV1Data, EventSessionRefusalDetectedV1DataKind as Kind,
    RecordSessionHandoffV2Trigger as Trigger, RecordTaskPacketV1,
};
use serde::{Deserialize, Deserializer, de};

use crate::budget::BudgetDecision;
use crate::handoff::{Consent, Decision, HandoffPolicy};
use crate::routing::{self, Candidate, QuotaObservation, Route, RoutingPolicy, TaskProfile};

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/refusal.toml");

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    AnyTool,
    SameTool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Order {
    Routing,
    OtherToolFirst,
    SameToolFirst,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefusalPolicy {
    enabled: bool,
    scope: Scope,
    order: Order,
    #[serde(deserialize_with = "deserialize_max")]
    max_alternatives: u8,
}

/// Routing inputs for the refused task, as the router would receive them.
pub struct RouteContext<'a> {
    pub routing: &'a RoutingPolicy,
    pub task: &'a RecordTaskPacketV1,
    pub profile: &'a TaskProfile<'a>,
    pub candidates: &'a [Candidate],
    pub quota: &'a [QuotaObservation],
    pub now_secs: u64,
    pub budget: &'a BudgetDecision,
}

/// A proposal only: nothing runs until a device of the owner accepts the handoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub handoff_id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    pub trigger: Trigger,
    pub kind: Kind,
    pub alternatives: Vec<Route>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptError {
    NotAccepted,
    NotDevice,
    OtherHandoff,
    OtherSession,
    NotOffered,
    Policy,
}

impl RefusalPolicy {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    /// `refusals` holds every refusal of the same work, oldest first; the last one triggers the offer.
    /// A harness and model that already refused is never offered again.
    pub fn offer(
        &self,
        handoff: &HandoffPolicy,
        handoff_id: CommonIdV1,
        refusals: &[EventSessionRefusalDetectedV1Data],
        context: &RouteContext<'_>,
    ) -> Option<Offer> {
        let last = refusals.last()?;
        if !self.enabled
            || handoff.evaluate(Trigger::ModelRefusal, None, Consent::Pending) != Decision::Propose
            || matches!(context.budget, BudgetDecision::Stop(_))
        {
            return None;
        }

        let refused = |route: &Route| {
            refusals.iter().any(|refusal| {
                refusal.harness == route.harness && refusal.model.as_str() == route.model.as_str()
            })
        };
        let mut alternatives: Vec<&Route> = Vec::new();
        for candidate in routing::ranked(
            context.routing,
            context.task,
            context.profile,
            context.candidates,
        ) {
            let route = &candidate.route;
            if refused(route)
                || (self.scope == Scope::SameTool && route.harness != last.harness)
                || !routing::available(context.routing, route, context.quota, context.now_secs)
                || alternatives.iter().any(|seen| {
                    seen.harness == route.harness
                        && seen.provider == route.provider
                        && seen.model == route.model
                })
            {
                continue;
            }
            alternatives.push(route);
        }
        match self.order {
            Order::Routing => {}
            Order::OtherToolFirst => {
                alternatives.sort_by_key(|route| route.harness == last.harness)
            }
            Order::SameToolFirst => alternatives.sort_by_key(|route| route.harness != last.harness),
        }
        alternatives.truncate(usize::from(self.max_alternatives));
        if alternatives.is_empty() {
            return None;
        }
        Some(Offer {
            handoff_id,
            source_session_id: last.session_id.clone(),
            trigger: Trigger::ModelRefusal,
            kind: last.kind,
            alternatives: alternatives.into_iter().cloned().collect(),
        })
    }
}

impl Offer {
    /// The caller must also check that the accepting device belongs to the owner of the lineage.
    pub fn accept(
        &self,
        handoff: &HandoffPolicy,
        consent: &EventSessionHandoffV2,
        choice: &Route,
    ) -> Result<Route, AcceptError> {
        if consent.data.status != EventSessionHandoffV2DataStatus::Accepted {
            return Err(AcceptError::NotAccepted);
        }
        if consent.actor.kind != CommonActorV1Kind::Device {
            return Err(AcceptError::NotDevice);
        }
        if consent.data.handoff_id != self.handoff_id {
            return Err(AcceptError::OtherHandoff);
        }
        if consent.data.source_session_id != self.source_session_id {
            return Err(AcceptError::OtherSession);
        }
        if !self.alternatives.contains(choice) {
            return Err(AcceptError::NotOffered);
        }
        if handoff.evaluate(self.trigger, None, Consent::Granted) != Decision::Proceed {
            return Err(AcceptError::Policy);
        }
        Ok(choice.clone())
    }
}

fn deserialize_max<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let max = u8::deserialize(deserializer)?;
    if (1..=10).contains(&max) {
        Ok(max)
    } else {
        Err(de::Error::custom(
            "max_alternatives must be between 1 and 10",
        ))
    }
}
