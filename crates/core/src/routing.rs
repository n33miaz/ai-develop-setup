//! Select a configured route using an ordered table and observed quota.

use ads_contracts::{
    CommonDataClassV1, CommonIdV1, ContractError, EventRoutingDecidedV1Data,
    EventRoutingDecidedV1DataEffort, EventRoutingDecidedV1DataMode, EventRoutingDecidedV1DataModel,
    EventRoutingDecidedV1DataTier, EventRoutingFallbackV1Data, EventRoutingFallbackV1DataErrorCode,
    EventRoutingFallbackV1DataFrom, EventRoutingFallbackV1DataFromModel,
    EventRoutingFallbackV1DataTo, EventRoutingFallbackV1DataToModel, EventRoutingProposedV1Data,
    EventRoutingProposedV1DataEffort, EventRoutingProposedV1DataMode as Mode,
    EventRoutingProposedV1DataModel, EventRoutingProposedV1DataTier, RecordTaskPacketV1,
    RecordTaskPacketV1BudgetQuotaPct, RecordUsageEventV1QuotaWindowAfter,
};
use serde::Deserialize;

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/routing.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingPolicy {
    mode: Mode,
    quota_max_age_secs: u64,
    quota_block_at_pct: RecordTaskPacketV1BudgetQuotaPct,
    allow_unknown_quota: bool,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: CommonIdV1,
    kind: Option<CommonIdV1>,
    size: Option<Size>,
    sensitivity: Option<CommonDataClassV1>,
    tiers: Option<Vec<EventRoutingProposedV1DataTier>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    Small,
    Medium,
    Large,
}

pub struct TaskProfile<'a> {
    pub kind: &'a CommonIdV1,
    pub size: Size,
    pub sensitivity: CommonDataClassV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingProposedV1DataModel,
    pub effort: Option<EventRoutingProposedV1DataEffort>,
    pub tier: EventRoutingProposedV1DataTier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: CommonIdV1,
    pub rank: u32,
    pub allowed_sensitivity: Vec<CommonDataClassV1>,
    pub route: Route,
}

pub enum QuotaState {
    Measured(RecordUsageEventV1QuotaWindowAfter),
    RateLimited,
}

pub struct QuotaObservation {
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub observed_at_secs: u64,
    pub state: QuotaState,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RoutingOutcome {
    pub proposed: Option<EventRoutingProposedV1Data>,
    pub decided: Option<EventRoutingDecidedV1Data>,
    /// A counterfactual fallback in shadow and propose modes.
    pub fallback: Option<EventRoutingFallbackV1Data>,
}

/// Compute event payloads without applying a route.
/// Quota observations must be scoped to the current account binding for each harness and provider.
pub fn evaluate(
    policy: &RoutingPolicy,
    task: &RecordTaskPacketV1,
    profile: &TaskProfile<'_>,
    candidates: &[Candidate],
    manual: Option<&Route>,
    quota: &[QuotaObservation],
    now_secs: u64,
) -> Result<RoutingOutcome, ContractError> {
    let mode = match task.mode.as_str() {
        "manual" => Mode::Manual,
        "propose" => Mode::Propose,
        "auto" => Mode::Auto,
        "shadow" => Mode::Shadow,
        _ => policy.mode,
    };
    let manual = manual.filter(|route| {
        candidates
            .iter()
            .any(|candidate| &candidate.route == *route)
    });
    let mut outcome = RoutingOutcome::default();
    if mode != Mode::Auto {
        outcome.decided = manual
            .map(|route| decided(task, mode, None, route))
            .transpose()?;
    }
    if mode == Mode::Manual {
        return Ok(outcome);
    }

    let Some(rule) = policy.rules.iter().find(|rule| {
        rule.kind.as_ref().is_none_or(|kind| kind == profile.kind)
            && rule.size.is_none_or(|size| size == profile.size)
            && rule
                .sensitivity
                .is_none_or(|class| class == profile.sensitivity)
    }) else {
        return Ok(outcome);
    };
    let mut ranked: Vec<_> = candidates
        .iter()
        .filter_map(|candidate| {
            if !candidate.allowed_sensitivity.contains(&profile.sensitivity) {
                return None;
            }
            let tier = candidate.route.tier.as_str();
            let tier_rank = match &rule.tiers {
                Some(tiers) => tiers
                    .iter()
                    .position(|preferred| preferred.as_str() == tier)?,
                None if tier == task.tier_requested.as_str() => 0,
                None => return None,
            };
            Some((tier_rank, candidate))
        })
        .collect();
    ranked.sort_unstable_by_key(|&(tier_rank, candidate)| {
        let route = &candidate.route;
        (
            tier_rank,
            candidate.rank,
            candidate.id.as_str(),
            route.harness.as_str(),
            route.provider.as_str(),
            route.model.as_str(),
            route.effort.as_ref().map(|effort| effort.as_str()),
        )
    });
    let Some((_, preferred)) = ranked.first() else {
        return Ok(outcome);
    };
    let Some((_, selected)) = ranked
        .iter()
        .find(|(_, candidate)| quota_block(policy, &candidate.route, quota, now_secs).is_none())
    else {
        return Ok(outcome);
    };

    let route = &selected.route;
    outcome.proposed = Some(EventRoutingProposedV1Data {
        task_id: task.task_id.clone(),
        mode,
        rule_id: Some(rule.id.clone()),
        tier: route.tier.clone(),
        harness: route.harness.clone(),
        provider: route.provider.clone(),
        model: route.model.clone(),
        effort: route.effort.clone(),
    });
    if mode == Mode::Auto {
        outcome.decided = Some(decided(task, mode, Some(rule.id.clone()), route)?);
    }
    if let Some(code) = quota_block(policy, &preferred.route, quota, now_secs) {
        outcome.fallback = Some(fallback(task, &preferred.route, route, code)?);
    }
    Ok(outcome)
}

fn quota_block(
    policy: &RoutingPolicy,
    route: &Route,
    quota: &[QuotaObservation],
    now: u64,
) -> Option<&'static str> {
    let latest = quota
        .iter()
        .filter_map(|observation| {
            let age = now.checked_sub(observation.observed_at_secs)?;
            if observation.harness != route.harness
                || observation.provider != route.provider
                || age > policy.quota_max_age_secs
            {
                return None;
            }
            let percent = match &observation.state {
                QuotaState::RateLimited => 100.0,
                QuotaState::Measured(window) => [
                    window
                        .five_hour_pct
                        .as_ref()
                        .and_then(|pct| pct.get().as_f64()),
                    window
                        .weekly_pct
                        .as_ref()
                        .and_then(|pct| pct.get().as_f64()),
                ]
                .into_iter()
                .flatten()
                .max_by(f64::total_cmp)?,
            };
            Some((observation.observed_at_secs, percent))
        })
        .max_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        });
    let cutoff = policy
        .quota_block_at_pct
        .get()
        .as_f64()
        .expect("checked percentage");
    match latest {
        Some((_, percent)) if percent >= cutoff => Some("E_RATE_LIMITED"),
        None if !policy.allow_unknown_quota => Some("E_QUOTA_UNKNOWN"),
        _ => None,
    }
}

fn decided(
    task: &RecordTaskPacketV1,
    mode: Mode,
    rule_id: Option<CommonIdV1>,
    route: &Route,
) -> Result<EventRoutingDecidedV1Data, ContractError> {
    Ok(EventRoutingDecidedV1Data {
        task_id: task.task_id.clone(),
        mode: match mode {
            Mode::Manual => EventRoutingDecidedV1DataMode::Manual,
            Mode::Propose => EventRoutingDecidedV1DataMode::Propose,
            Mode::Auto => EventRoutingDecidedV1DataMode::Auto,
            Mode::Shadow => EventRoutingDecidedV1DataMode::Shadow,
        },
        rule_id,
        tier: EventRoutingDecidedV1DataTier::new(route.tier.as_str())?,
        harness: route.harness.clone(),
        provider: route.provider.clone(),
        model: EventRoutingDecidedV1DataModel::new(route.model.as_str())?,
        effort: route
            .effort
            .as_ref()
            .map(|effort| EventRoutingDecidedV1DataEffort::new(effort.as_str()))
            .transpose()?,
    })
}

fn fallback(
    task: &RecordTaskPacketV1,
    from: &Route,
    to: &Route,
    code: &str,
) -> Result<EventRoutingFallbackV1Data, ContractError> {
    Ok(EventRoutingFallbackV1Data {
        task_id: task.task_id.clone(),
        from: EventRoutingFallbackV1DataFrom {
            harness: from.harness.clone(),
            provider: from.provider.clone(),
            model: EventRoutingFallbackV1DataFromModel::new(from.model.as_str())?,
        },
        to: EventRoutingFallbackV1DataTo {
            harness: to.harness.clone(),
            provider: to.provider.clone(),
            model: EventRoutingFallbackV1DataToModel::new(to.model.as_str())?,
        },
        error_code: EventRoutingFallbackV1DataErrorCode::new(code)?,
    })
}
