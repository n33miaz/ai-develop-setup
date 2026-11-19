//! Evaluate the limits of a task against its observed consumption.

use std::collections::HashSet;
use std::time::Duration;

use ads_contracts::{EventTaskCancelledV1DataReason, RecordTaskPacketV1, RecordUsageEventV1};
use serde::Deserialize;

pub const DEFAULT_POLICY: &str = include_str!("../../../defaults/budget.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetPolicy {
    warn_at_pct: ads_contracts::RecordTaskPacketV1BudgetQuotaPct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Usd,
    QuotaPct,
    Turns,
    WallTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    Continue,
    Warn(Vec<Dimension>),
    Stop(Vec<Dimension>),
}

impl BudgetDecision {
    pub fn cancellation_reason(&self) -> Option<EventTaskCancelledV1DataReason> {
        match self {
            Self::Stop(_) => Some(EventTaskCancelledV1DataReason::Budget),
            _ => None,
        }
    }
}

/// Use reconciled ledger rows for the task and elapsed wall time supplied by the caller.
/// Repeated identical rows count once. Rows without a matching task ID do not count.
pub fn evaluate(
    policy: &BudgetPolicy,
    task: &RecordTaskPacketV1,
    usage: &[RecordUsageEventV1],
    elapsed: Duration,
) -> BudgetDecision {
    let mut seen = HashSet::new();
    let mut costs = Vec::new();
    let mut quota_deltas = Vec::new();
    for row in usage
        .iter()
        .filter(|row| row.task_id.as_ref() == Some(&task.task_id))
    {
        if !seen.insert(row) {
            continue;
        }
        costs.push(
            row.cost_billable_usd
                .as_ref()
                .and_then(|cost| cost.get().as_f64())
                .unwrap_or_default(),
        );
        quota_deltas.push(quota_delta(row));
    }

    let limits = &task.budget;
    let elapsed_ns = elapsed.as_nanos();
    let limit_ns = u128::from(limits.wall_min.get()) * 60_000_000_000;
    let turns = seen.len() as u128;
    let turn_limit = u128::from(limits.turns.get());
    let usd = sum(costs);
    let quota = sum(quota_deltas);
    let usd_limit = limits.usd.get().as_f64().expect("checked USD limit");
    let quota_limit = limits
        .quota_pct
        .get()
        .as_f64()
        .expect("checked quota limit");
    let warning = policy
        .warn_at_pct
        .get()
        .as_f64()
        .expect("checked percentage")
        / 100.0;

    let dimensions = [
        (Dimension::Usd, usd >= usd_limit, usd >= usd_limit * warning),
        (
            Dimension::QuotaPct,
            quota >= quota_limit,
            quota >= quota_limit * warning,
        ),
        (
            Dimension::Turns,
            turns >= turn_limit,
            turns as f64 >= turn_limit as f64 * warning,
        ),
        (
            Dimension::WallTime,
            elapsed_ns >= limit_ns,
            elapsed_ns as f64 >= limit_ns as f64 * warning,
        ),
    ];

    let reached: Vec<_> = dimensions
        .iter()
        .filter_map(|&(dimension, stop, _)| stop.then_some(dimension))
        .collect();
    if !reached.is_empty() {
        return BudgetDecision::Stop(reached);
    }
    let warnings: Vec<_> = dimensions
        .iter()
        .filter_map(|&(dimension, _, warn)| warn.then_some(dimension))
        .collect();
    if warnings.is_empty() {
        BudgetDecision::Continue
    } else {
        BudgetDecision::Warn(warnings)
    }
}

fn quota_delta(row: &RecordUsageEventV1) -> f64 {
    let (Some(before), Some(after)) = (&row.quota_window_before, &row.quota_window_after) else {
        return 0.0;
    };
    let five_hour = delta(
        before
            .five_hour_pct
            .as_ref()
            .and_then(|pct| pct.get().as_f64()),
        after
            .five_hour_pct
            .as_ref()
            .and_then(|pct| pct.get().as_f64()),
    );
    let weekly = delta(
        before
            .weekly_pct
            .as_ref()
            .and_then(|pct| pct.get().as_f64()),
        after.weekly_pct.as_ref().and_then(|pct| pct.get().as_f64()),
    );
    five_hour.max(weekly)
}

fn delta(before: Option<f64>, after: Option<f64>) -> f64 {
    before
        .zip(after)
        .map_or(0.0, |(before, after)| (after - before).max(0.0))
}

fn sum(mut values: Vec<f64>) -> f64 {
    values.sort_unstable_by(f64::total_cmp);
    let mut total = 0.0;
    let mut correction = 0.0;
    for value in values {
        let adjusted = value - correction;
        let next = total + adjusted;
        if next.is_infinite() {
            return next;
        }
        correction = (next - total) - adjusted;
        total = next;
    }
    total
}
