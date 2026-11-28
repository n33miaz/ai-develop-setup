//! Offer other configured routes after a refusal or a safety filter.

use ads_contracts::{
    CommonIdV1, CommonTimestampV1, EventSessionHandoffV4, EventSessionHandoffV4DataStatus,
    EventSessionHandoffV4DataTrigger, EventSessionRefusalDetectedV1Data,
    EventSessionRefusalDetectedV1DataKind as Kind, EventSessionRefusalDetectedV1DataModel,
    RecordSessionHandoffV2, RecordSessionHandoffV2Trigger as Trigger, RecordTaskPacketV1,
};
use serde::{Deserialize, Deserializer, de};

use crate::budget::BudgetDecision;
use crate::device::{DeviceRefusal, check_owner_device};
use crate::handoff::{Consent, ConsentError, Decision, HandoffPolicy, check_consent_record};
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

/// One offered route, named by the id of its candidate in the catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alternative {
    pub id: CommonIdV1,
    pub route: Route,
}

/// A proposal only: nothing runs until a device of the lineage owner accepts one alternative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub handoff_id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    /// Owner of the lineage; only a device registered to this owner may accept.
    pub owner_id: CommonIdV1,
    pub trigger: Trigger,
    pub kind: Kind,
    pub alternatives: Vec<Alternative>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptError {
    NotAccepted,
    /// An agent or the daemon cannot accept.
    NotDevice,
    /// The device was not registered to the lineage owner when it accepted.
    NotOwnerDevice,
    OtherHandoff,
    OtherSession,
    OtherTrigger,
    /// The acceptance does not name the chosen alternative.
    NoChoice,
    /// The route about to start is not the one the device accepted.
    OtherRoute,
    NotOffered,
    Record(ConsentError),
    Policy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectError {
    /// No candidate of the catalog has this harness, provider and model.
    NotConfigured,
    /// The model of the catalog is not a short identifier the event can carry.
    Model,
}

/// Data of `session.refusal_detected:1` for a route of the configured catalog.
/// The model is a reference to the catalog, never text taken from the session.
pub fn detected(
    session_id: CommonIdV1,
    harness: &CommonIdV1,
    provider: &CommonIdV1,
    model: &str,
    kind: Kind,
    catalog: &[Candidate],
) -> Result<EventSessionRefusalDetectedV1Data, DetectError> {
    if !catalog.iter().any(|candidate| {
        candidate.route.harness == *harness
            && candidate.route.provider == *provider
            && candidate.route.model.as_str() == model
    }) {
        return Err(DetectError::NotConfigured);
    }
    Ok(EventSessionRefusalDetectedV1Data {
        session_id,
        harness: harness.clone(),
        provider: provider.clone(),
        model: EventSessionRefusalDetectedV1DataModel::new(model)
            .map_err(|_| DetectError::Model)?,
        kind,
    })
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
        owner_id: CommonIdV1,
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
        let mut alternatives: Vec<&Candidate> = Vec::new();
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
                    seen.route.harness == route.harness
                        && seen.route.provider == route.provider
                        && seen.route.model == route.model
                })
            {
                continue;
            }
            alternatives.push(candidate);
        }
        match self.order {
            Order::Routing => {}
            Order::OtherToolFirst => {
                alternatives.sort_by_key(|seen| seen.route.harness == last.harness)
            }
            Order::SameToolFirst => {
                alternatives.sort_by_key(|seen| seen.route.harness != last.harness)
            }
        }
        alternatives.truncate(usize::from(self.max_alternatives));
        if alternatives.is_empty() {
            return None;
        }
        Some(Offer {
            handoff_id,
            source_session_id: last.session_id.clone(),
            owner_id,
            trigger: Trigger::ModelRefusal,
            kind: last.kind,
            alternatives: alternatives
                .into_iter()
                .map(|candidate| Alternative {
                    id: candidate.id.clone(),
                    route: candidate.route.clone(),
                })
                .collect(),
        })
    }
}

impl Offer {
    /// Returns the route of `choice` only when `consent` is the acceptance of this handoff by a
    /// device of the lineage owner and names `choice`, one of the offered alternatives.
    /// The record must identify this acceptance, its owner, trigger and channel.
    /// `owner_at` reads the device registry at the instant of the acceptance, as the queue replay
    /// does: a device of another owner, an unknown or revoked device, an agent and the daemon
    /// cannot accept.
    pub fn accept(
        &self,
        handoff: &HandoffPolicy,
        record: &RecordSessionHandoffV2,
        consent: &EventSessionHandoffV4,
        choice: &CommonIdV1,
        owner_at: impl Fn(&CommonIdV1, &CommonTimestampV1) -> Option<CommonIdV1>,
    ) -> Result<Route, AcceptError> {
        if consent.data.status != EventSessionHandoffV4DataStatus::Accepted {
            return Err(AcceptError::NotAccepted);
        }
        check_owner_device(&consent.actor, &consent.ts, &self.owner_id, owner_at).map_err(
            |refusal| match refusal {
                DeviceRefusal::NotDevice => AcceptError::NotDevice,
                DeviceRefusal::NotOwnerDevice => AcceptError::NotOwnerDevice,
            },
        )?;
        let record_consent = check_consent_record(record, &self.owner_id, consent);
        // A record identity error can mask another mismatch. Check both consent IDs against
        // the offer in that case so the public offer errors still take precedence.
        let (handoff_id, source_session_id) = match record_consent {
            Err(ConsentError::OtherHandoff | ConsentError::OtherSession) => {
                (&consent.data.handoff_id, &consent.data.source_session_id)
            }
            _ => (&record.id, &record.source_session_id),
        };
        if handoff_id != &self.handoff_id {
            return Err(AcceptError::OtherHandoff);
        }
        if source_session_id != &self.source_session_id {
            return Err(AcceptError::OtherSession);
        }
        let Some(accepted) = consent.data.alternative_id.as_ref() else {
            return Err(AcceptError::NoChoice);
        };
        if self.trigger != Trigger::ModelRefusal
            || consent.data.trigger != EventSessionHandoffV4DataTrigger::ModelRefusal
        {
            return Err(AcceptError::OtherTrigger);
        }
        record_consent.map_err(AcceptError::Record)?;
        if accepted != choice {
            return Err(AcceptError::OtherRoute);
        }
        let Some(alternative) = self
            .alternatives
            .iter()
            .find(|alternative| alternative.id == *accepted)
        else {
            return Err(AcceptError::NotOffered);
        };
        if handoff.evaluate(self.trigger, None, Consent::Granted) != Decision::Proceed {
            return Err(AcceptError::Policy);
        }
        Ok(alternative.route.clone())
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
