use ads_core::spend::{Month, Refusal, SpendGuard, SpendLimit};

fn month(m: u8) -> Month {
    Month::new(2030, m).unwrap()
}

fn capped(micro_usd: u64) -> SpendLimit {
    SpendLimit {
        enabled: true,
        monthly_cap_micro_usd: Some(micro_usd),
    }
}

#[test]
fn months_outside_one_to_twelve_are_refused() {
    assert!(Month::new(2030, 0).is_none());
    assert!(Month::new(2030, 13).is_none());
    assert!(Month::new(2030, 1) < Month::new(2030, 2));
    assert!(Month::new(2029, 12) < Month::new(2030, 1));
}

#[test]
fn the_default_limit_is_off_and_refuses_before_the_cap() {
    let limit = SpendLimit::default();
    assert!(!limit.enabled);
    assert_eq!(limit.monthly_cap_micro_usd, None);
    let mut guard = SpendGuard::new(limit, month(1), 0);
    assert_eq!(guard.authorize(month(1), 0).unwrap_err(), Refusal::Disabled);
    let mut off = SpendGuard::new(
        SpendLimit {
            enabled: false,
            monthly_cap_micro_usd: Some(1_000_000),
        },
        month(1),
        0,
    );
    assert_eq!(off.authorize(month(1), 1).unwrap_err(), Refusal::Disabled);
    let mut uncapped = SpendGuard::new(
        SpendLimit {
            enabled: true,
            monthly_cap_micro_usd: None,
        },
        month(1),
        0,
    );
    assert_eq!(uncapped.authorize(month(1), 0).unwrap_err(), Refusal::NoCap);
}

#[test]
fn holds_count_against_the_cap_until_settled_or_released() {
    let mut guard = SpendGuard::new(capped(1_000), month(1), 200);
    assert_eq!(guard.month(), month(1));
    assert_eq!(guard.spent_micro_usd(), 200);
    assert_eq!(
        guard.authorize(month(1), 801).unwrap_err(),
        Refusal::WouldExceedCap
    );
    let held = guard.authorize(month(1), 500).expect("fits");
    assert_eq!(held.micro_usd(), 500);
    assert_eq!(
        guard.authorize(month(1), 301).unwrap_err(),
        Refusal::WouldExceedCap
    );
    guard.release(held).expect("own");
    assert_eq!(guard.spent_micro_usd(), 200);
    let held = guard.authorize(month(1), 800).expect("fits after release");
    guard.settle(held, 900).expect("own");
    assert_eq!(guard.spent_micro_usd(), 1_100);
    assert_eq!(
        guard.authorize(month(1), 0).unwrap_err(),
        Refusal::CapReached
    );
}

#[test]
fn reservations_cannot_overflow_the_maximum_cap() {
    let now = month(1);
    let mut guard = SpendGuard::new(capped(u64::MAX), now, 0);
    let held = guard.authorize(now, u64::MAX - 1).expect("fits");

    assert_eq!(
        guard.authorize(now, 2).unwrap_err(),
        Refusal::WouldExceedCap
    );

    let remaining = guard.authorize(now, 1).expect("remaining capacity fits");
    assert_eq!(guard.authorize(now, 1).unwrap_err(), Refusal::CapReached);
    guard.release(remaining).expect("own");
    guard.release(held).expect("own");
}

#[test]
fn a_later_month_starts_empty_and_an_earlier_one_counts_in_the_guard_month() {
    let mut guard = SpendGuard::new(capped(1_000), month(5), 1_000);
    assert_eq!(
        guard.authorize(month(4), 0).unwrap_err(),
        Refusal::CapReached
    );
    assert_eq!(guard.month(), month(5));
    let old = guard.authorize(month(6), 400).expect("new month");
    assert_eq!(guard.month(), month(6));
    assert_eq!(guard.spent_micro_usd(), 0);
    let next = guard.authorize(month(7), 1_000).expect("next month");
    // A hold from a month the guard left changes nothing when it finishes.
    guard.settle(old, 400).expect("own");
    assert_eq!(guard.spent_micro_usd(), 0);
    assert_eq!(
        guard.authorize(month(7), 1).unwrap_err(),
        Refusal::CapReached
    );
    guard.settle(next, 300).expect("own");
    assert_eq!(guard.spent_micro_usd(), 300);
}

#[test]
fn a_reservation_from_another_guard_comes_back_untouched() {
    let mut a = SpendGuard::new(capped(100), month(1), 0);
    let mut b = SpendGuard::new(capped(100), month(1), 0);
    let from_a = a.authorize(month(1), 100).expect("fits");
    let returned = b.settle(from_a, 100).unwrap_err().into_reservation();
    assert_eq!(b.spent_micro_usd(), 0);
    assert_eq!(returned.micro_usd(), 100);
    a.release(returned).expect("issuer");
    assert!(a.authorize(month(1), 100).is_ok());
}

#[test]
fn refusals_name_no_adapter() {
    for refusal in [
        Refusal::Disabled,
        Refusal::NoCap,
        Refusal::CapReached,
        Refusal::WouldExceedCap,
    ] {
        assert!(!refusal.to_string().contains("api"), "{refusal}");
    }
}
