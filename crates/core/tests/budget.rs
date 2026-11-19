use std::time::Duration;

use ads_contracts::{EventTaskCancelledV1DataReason, RecordTaskPacketV1, RecordUsageEventV1};
use ads_core::budget::{BudgetDecision, BudgetPolicy, DEFAULT_POLICY, Dimension, evaluate};
use serde_json::{Value, json};

const TASK: &str =
    include_str!("../../../contracts/golden/record.task-packet.v1/valid/01-base.json");
const USAGE: &str =
    include_str!("../../../contracts/golden/record.usage_event.v1/valid/01-base.json");

fn task(usd: f64, quota: u64, turns: u64, minutes: u64) -> RecordTaskPacketV1 {
    let mut value: Value = serde_json::from_str(TASK).unwrap();
    value["budget"] = json!({"usd": usd, "quota_pct": quota, "turns": turns, "wall_min": minutes});
    serde_json::from_value(value).unwrap()
}

fn usage(index: usize, cost: f64, quota: u64) -> RecordUsageEventV1 {
    let mut value: Value = serde_json::from_str(USAGE).unwrap();
    value["ts"] = json!(format!("2030-01-02T03:04:{index:02}Z"));
    value["event_id"] = json!(format!("usage-{index}"));
    value["dedupe_key"] = json!(format!("key-{index}"));
    value["cost_billable_usd"] = json!(cost);
    value["quota_window_after"]["five_hour_pct"] = json!(10 + quota);
    value["quota_window_after"]["weekly_pct"] = json!(2);
    serde_json::from_value(value).unwrap()
}

#[test]
fn each_dimension_continues_warns_and_stops_at_the_boundary() {
    let policy: BudgetPolicy = toml::from_str(DEFAULT_POLICY).unwrap();
    let cases = [
        (Dimension::Usd, task(5.0, 100, 20, 20)),
        (Dimension::QuotaPct, task(100.0, 5, 20, 20)),
        (Dimension::Turns, task(100.0, 100, 5, 20)),
        (Dimension::WallTime, task(100.0, 100, 20, 5)),
    ];
    for (dimension, task) in cases {
        for (count, expected) in [
            (3, BudgetDecision::Continue),
            (4, BudgetDecision::Warn(vec![dimension])),
            (5, BudgetDecision::Stop(vec![dimension])),
            (6, BudgetDecision::Stop(vec![dimension])),
        ] {
            let rows: Vec<_> = (0..count).map(|index| usage(index, 1.0, 1)).collect();
            let result = evaluate(
                &policy,
                &task,
                &rows,
                Duration::from_secs(count as u64 * 60),
            );
            assert_eq!(result, expected);
            assert_eq!(
                result.cancellation_reason(),
                (count >= 5).then_some(EventTaskCancelledV1DataReason::Budget)
            );
        }
    }
}

#[test]
fn ledger_order_and_repeated_identical_rows_do_not_change_decision() {
    let policy = toml::from_str(DEFAULT_POLICY).unwrap();
    let task = task(0.3, 5, 4, 20);
    let mut rows = vec![usage(1, 0.1, 1), usage(2, 0.2, 1)];
    let mut unrelated = usage(3, 50.0, 50).into_fields();
    unrelated.task_id = None;
    rows.push(RecordUsageEventV1::new(unrelated).unwrap());
    let expected = BudgetDecision::Stop(vec![Dimension::Usd]);
    assert_eq!(evaluate(&policy, &task, &rows, Duration::ZERO), expected);
    rows.push(rows[0].clone());
    rows.reverse();
    assert_eq!(evaluate(&policy, &task, &rows, Duration::ZERO), expected);
}

#[test]
fn estimated_list_cost_does_not_count_toward_billable_usd() {
    let policy = toml::from_str(DEFAULT_POLICY).unwrap();
    let task = task(0.001, 100, 20, 20);
    let row = serde_json::from_str(USAGE).unwrap();
    assert_eq!(
        evaluate(&policy, &task, &[row], Duration::ZERO),
        BudgetDecision::Continue
    );
}

#[test]
fn invalid_warning_percentage_is_rejected() {
    assert!(toml::from_str::<BudgetPolicy>("warn_at_pct = 101.0").is_err());
}
