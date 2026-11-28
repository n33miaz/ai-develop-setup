use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use ads_adapter_api::{
    ApiConfig, CallError, CallOutcome, ChatRequest, MAX_RESPONSE_BYTES, MapError, Message, Month,
    PreparedRequest, ProviderConfig, Refusal, Role, SpendGuard, Transport, TransportError,
    TransportResponse, build_request, run_call,
};
use ads_contracts::{RecordApiUsageV3CostBasis, RecordApiUsageV3Outcome};

const TIMESTAMP: &str = "2030-01-02T03:04:05Z";
const NEXT_TIMESTAMP: &str = "2030-02-01T00:00:00Z";
const PROMPT: &str = "very-private-prompt-text";
const KEY_ALIAS: &str = "vault-entry";
const WAIT: Duration = Duration::from_secs(5);
// 3 tokens in at 2 USD and 4 out at 8 USD per million.
const REPORTED_COST: u64 = 38;
const REPORTED: &str = r#"{"choices":[{"message":{"content":"very-private-answer"},"finish_reason":"very-private-reason"}],"usage":{"prompt_tokens":3,"completion_tokens":4}}"#;

type Reply = Result<TransportResponse, TransportError>;

struct Fake {
    reply: Reply,
    calls: AtomicUsize,
    keys: Mutex<Vec<String>>,
}

impl Fake {
    fn new(reply: Reply) -> Self {
        Self {
            reply,
            calls: AtomicUsize::new(0),
            keys: Mutex::new(Vec::new()),
        }
    }

    fn answering(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self::new(Ok(TransportResponse {
            status,
            body: body.into(),
        }))
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Transport for Fake {
    fn send(&self, request: &PreparedRequest) -> Reply {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let key = request.key.handle.reference().to_owned();
        self.keys.lock().unwrap().push(key);
        self.reply.clone()
    }
}

fn provider() -> ProviderConfig {
    let toml = format!(
        r#"
[providers.provider-a]
base_url = "https://api.example.com/v1"
model = "model-x"
input_usd_per_mtok = 2.0
output_usd_per_mtok = 8.0
price_version = "prices-2030-01-01"
key_aliases = ["{KEY_ALIAS}", "spare"]
active_key = "{KEY_ALIAS}"
"#
    );
    let config = ApiConfig::from_toml(&toml).expect("valid config");
    config.provider("provider-a").expect("provider").clone()
}

fn request() -> ChatRequest {
    ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: PROMPT.to_owned(),
        }],
        max_tokens: Some(100),
        temperature: None,
    }
}

#[test]
fn oversized_request_is_refused_before_transport() {
    let provider = provider();
    let transport = Fake::answering(200, REPORTED);
    let cap = 10_000_000;
    let guard = guard(cap);
    for content in ["\0".repeat(200_000), "a".repeat(1024 * 1024 + 1)] {
        let mut request = request();
        request.messages[0].content = content;
        let outcome = run_call(&transport, &guard, &provider, &request, month(), TIMESTAMP);

        assert_eq!(transport.calls(), 0);
        assert_eq!(
            outcome.result,
            Err(CallError::Request(MapError::RequestTooLarge))
        );
        assert_eq!(outcome.spent_micro_usd, 0);
        assert!(outcome.record.is_none());
        assert_free(&guard, cap);
    }
}

/// The amount one `request()` reserves.
fn worst() -> u64 {
    let prepared = build_request(&provider(), &request()).expect("valid request");
    prepared.worst_case_micro_usd
}

fn month() -> Month {
    Month::new(2030, 1).expect("month")
}

fn next_month() -> Month {
    Month::new(2030, 2).expect("month")
}

fn spend_guard(cap_micro_usd: u64) -> SpendGuard {
    let config = ApiConfig::from_toml(&format!(
        "enabled = true\nmonthly_cap_usd = {cap_micro_usd}e-6"
    ))
    .expect("valid config");
    assert_eq!(config.monthly_cap_micro_usd(), Some(cap_micro_usd));
    SpendGuard::new(&config, month(), 0)
}

fn guard(cap_micro_usd: u64) -> Mutex<SpendGuard> {
    Mutex::new(spend_guard(cap_micro_usd))
}

fn call(transport: &dyn Transport, guard: &Mutex<SpendGuard>) -> CallOutcome {
    run_call(
        transport,
        guard,
        &provider(),
        &request(),
        month(),
        TIMESTAMP,
    )
}

/// Whether exactly `free` still fits under the cap: no more spent or held than expected.
fn assert_free(guard: &Mutex<SpendGuard>, free: u64) {
    assert_free_in(guard, month(), free);
}

fn assert_free_in(guard: &Mutex<SpendGuard>, now: Month, free: u64) {
    let mut guard = guard.lock().unwrap_or_else(PoisonError::into_inner);
    let over = guard.authorize(now, free + 1);
    assert!(over.is_err(), "more than {free} is free");
    let exact = guard.authorize(now, free).expect("exactly this is free");
    guard.release(exact).expect("own");
}

/// One attempt with the active key, charged at the reservation, with nothing left on hold.
fn assert_charged_reserve(
    fake: Fake,
    expected: CallError,
    expected_outcome: RecordApiUsageV3Outcome,
) {
    let guard = guard(3 * worst());
    let outcome = call(&fake, &guard);

    assert_eq!(outcome.result, Err(expected));
    assert_eq!(outcome.spent_micro_usd, worst());
    let record = outcome.record.expect("a record");
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reserved);
    assert_eq!(record.cost_estimate_micro_usd.get(), worst());
    assert_eq!((record.tokens_in.get(), record.tokens_out.get()), (0, 0));
    assert_eq!(record.outcome, expected_outcome);
    assert_eq!(fake.calls(), 1);
    assert_eq!(
        *fake.keys.lock().unwrap(),
        [format!("provider-a/{KEY_ALIAS}")]
    );
    assert_free(&guard, 2 * worst());
}

#[test]
fn tariff_precision_bounds_the_reservation_and_settles_reported_tokens() {
    let cfg = ApiConfig::from_toml(
        r#"
[providers.precision]
base_url = "https://api.example.com"
model = "model-x"
input_usd_per_mtok = 0
output_usd_per_mtok = 0.000001
max_output_tokens = 1000001
price_version = "p"
"#,
    )
    .unwrap();
    let provider = cfg.provider("precision").unwrap();
    let mut req = request();
    req.max_tokens = None;
    let fake = Fake::answering(
        200,
        r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":1,"completion_tokens":999999}}"#,
    );
    let denied = run_call(&fake, &guard(1), provider, &req, month(), TIMESTAMP);
    assert_eq!(
        denied.result,
        Err(CallError::Refused(Refusal::WouldExceedCap))
    );
    assert_eq!(fake.calls(), 0);

    let funds = guard(2);
    let billed = run_call(&fake, &funds, provider, &req, month(), TIMESTAMP);
    assert!(billed.result.is_ok());
    assert_eq!(billed.spent_micro_usd, 1);
    let record = billed.record.unwrap();
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reported);
    assert_eq!(record.cost_estimate_micro_usd.get(), 1);
    assert_free(&funds, 1);
}

#[test]
fn a_request_that_cannot_be_built_is_not_sent_and_holds_nothing() {
    let fake = Fake::answering(200, REPORTED);
    let guard = guard(worst());
    let mut empty = request();
    empty.messages.clear();
    let outcome = run_call(&fake, &guard, &provider(), &empty, month(), TIMESTAMP);

    assert_eq!(
        outcome.result,
        Err(CallError::Request(MapError::EmptyMessages))
    );
    assert_eq!((outcome.spent_micro_usd, outcome.record), (0, None));
    assert_eq!(fake.calls(), 0);
    assert_free(&guard, worst());
}

#[test]
fn a_timestamp_the_record_refuses_stops_the_call_before_it_is_sent() {
    let fake = Fake::answering(200, REPORTED);
    let guard = guard(worst());
    let outcome = run_call(&fake, &guard, &provider(), &request(), month(), "yesterday");

    assert_eq!(outcome.result, Err(CallError::Record));
    assert_eq!((outcome.spent_micro_usd, outcome.record), (0, None));
    assert_eq!(fake.calls(), 0);
    assert_free(&guard, worst());
}

#[test]
fn a_timestamp_outside_the_month_of_the_call_stops_it_before_it_is_sent() {
    let fake = Fake::answering(200, REPORTED);
    let guard = guard(worst());
    for timestamp in [NEXT_TIMESTAMP, "2029-01-02T03:04:05Z"] {
        let outcome = run_call(&fake, &guard, &provider(), &request(), month(), timestamp);

        assert_eq!(outcome.result, Err(CallError::Record));
        assert_eq!((outcome.spent_micro_usd, outcome.record), (0, None));
    }
    assert_eq!(fake.calls(), 0);
    assert_free(&guard, worst());
}

#[test]
fn a_call_behind_the_guard_month_is_not_sent_and_holds_nothing() {
    let fake = Fake::answering(200, REPORTED);
    let guard = guard(worst());
    // Another reservation has already moved the guard to the next month.
    assert_free_in(&guard, next_month(), worst());
    let outcome = call(&fake, &guard);

    assert_eq!(outcome.result, Err(CallError::MonthPassed));
    assert_eq!((outcome.spent_micro_usd, outcome.record), (0, None));
    assert_eq!(fake.calls(), 0);
    assert_free_in(&guard, next_month(), worst());
}

#[test]
fn a_refused_call_never_reaches_the_transport() {
    let fake = Fake::answering(200, REPORTED);
    let small = guard(worst() - 1);
    let outcome = call(&fake, &small);
    assert_eq!(
        outcome.result,
        Err(CallError::Refused(Refusal::WouldExceedCap))
    );
    assert_eq!((outcome.spent_micro_usd, outcome.record), (0, None));
    assert_free(&small, worst() - 1);

    let disabled = Mutex::new(SpendGuard::new(&ApiConfig::defaults(), month(), 7));
    let outcome = call(&fake, &disabled);
    assert_eq!(outcome.result, Err(CallError::Refused(Refusal::Disabled)));
    assert_eq!((outcome.spent_micro_usd, outcome.record), (7, None));

    assert_eq!(fake.calls(), 0);
}

#[test]
fn a_request_that_certainly_did_not_leave_releases_the_reservation() {
    let fake = Fake::new(Err(TransportError::NotSent));
    let guard = guard(worst());
    let outcome = call(&fake, &guard);

    assert_eq!(outcome.result, Err(CallError::NotSent));
    assert_eq!((outcome.spent_micro_usd, outcome.record), (0, None));
    assert_eq!(fake.calls(), 1);
    assert_free(&guard, worst());
}

#[test]
fn a_failure_after_sending_charges_the_reservation() {
    assert_charged_reserve(
        Fake::new(Err(TransportError::MaybeSent)),
        CallError::Uncertain,
        RecordApiUsageV3Outcome::ChargedUncertain,
    );
}

#[test]
fn a_status_outside_2xx_charges_the_reservation_even_with_reported_usage() {
    for status in [199, 300, 307, 401, 429, 500] {
        assert_charged_reserve(
            Fake::answering(status, REPORTED),
            CallError::Status(status),
            RecordApiUsageV3Outcome::ChargedErrorStatus,
        );
    }
}

#[test]
fn an_oversized_body_charges_the_reservation() {
    // Valid and with usage, so only the size can refuse it.
    let mut body = REPORTED.as_bytes().to_vec();
    body.resize(MAX_RESPONSE_BYTES + 1, b' ');
    assert_charged_reserve(
        Fake::answering(200, body),
        CallError::Response(MapError::ResponseTooLarge),
        RecordApiUsageV3Outcome::ChargedBadResponse,
    );
}

#[test]
fn an_unreadable_body_charges_the_reservation() {
    for body in [&b"not json"[..], &[0xff, 0xfe], b"", br#"{"choices":[]}"#] {
        assert_charged_reserve(
            Fake::answering(200, body),
            CallError::Response(MapError::MalformedResponse),
            RecordApiUsageV3Outcome::ChargedBadResponse,
        );
    }
}

#[test]
fn a_success_settles_the_reported_usage_and_frees_the_rest() {
    let fake = Fake::answering(200, REPORTED);
    let guard = guard(worst());
    let outcome = call(&fake, &guard);

    assert_eq!(
        outcome.result.expect("response").text,
        "very-private-answer"
    );
    assert_eq!(outcome.spent_micro_usd, REPORTED_COST);
    let record = outcome.record.expect("a record");
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reported);
    assert_eq!(record.cost_estimate_micro_usd.get(), REPORTED_COST);
    assert_eq!((record.tokens_in.get(), record.tokens_out.get()), (3, 4));
    assert_eq!(record.outcome, RecordApiUsageV3Outcome::SucceededReported);
    assert_eq!(record.ts.to_string(), TIMESTAMP);
    assert_eq!(fake.calls(), 1);
    assert_free(&guard, worst() - REPORTED_COST);
}

#[test]
fn a_success_without_usage_costs_the_reservation() {
    // The last status that still counts as success.
    let fake = Fake::answering(299, r#"{"choices":[{"message":{"content":"hi"}}]}"#);
    let guard = guard(2 * worst());
    let outcome = call(&fake, &guard);

    assert_eq!(outcome.result.expect("response").text, "hi");
    assert_eq!(outcome.spent_micro_usd, worst());
    let record = outcome.record.expect("a record");
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reserved);
    assert_eq!(record.outcome, RecordApiUsageV3Outcome::SucceededReserved);
    assert_free(&guard, worst());
}

fn answering_counts(tokens_in: u64, tokens_out: u64) -> Fake {
    let body = format!(
        r#"{{"choices":[{{"message":{{"content":"hi"}}}}],"usage":{{"prompt_tokens":{tokens_in},"completion_tokens":{tokens_out}}}}}"#
    );
    Fake::answering(200, body)
}

#[test]
fn token_counts_and_cost_at_the_limit_are_charged_as_reported() {
    let limit = 9_007_199_254_740_991;
    let mut provider = provider();
    provider.input_micro_usd_per_mtok = 1_000_000;
    provider.output_micro_usd_per_mtok = 0;
    let request = request();
    let reserved = build_request(&provider, &request)
        .unwrap()
        .worst_case_micro_usd;
    let funds = guard(reserved);
    let outcome = run_call(
        &answering_counts(limit, limit),
        &funds,
        &provider,
        &request,
        month(),
        TIMESTAMP,
    );

    assert!(outcome.result.unwrap().usage.is_some());
    assert_eq!(outcome.spent_micro_usd, limit);
    let record = outcome.record.expect("a record");
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reported);
    assert_eq!(record.cost_estimate_micro_usd.get(), limit);
    assert_eq!(
        (record.tokens_in.get(), record.tokens_out.get()),
        (limit, limit)
    );
    assert_eq!(record.outcome, RecordApiUsageV3Outcome::SucceededReported);
}

#[test]
fn a_reported_cost_above_the_limit_uses_the_reservation() {
    let limit = 9_007_199_254_740_991;
    // 2 micro-dollars per input token makes the next even cost exceed the limit.
    let funds = guard(2 * worst());
    let above = call(&answering_counts(limit / 2 + 1, 0), &funds);
    assert_eq!(above.spent_micro_usd, worst());
    let record = above.record.unwrap();
    assert_eq!(record.cost_estimate_micro_usd.get(), worst());
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reserved);
    assert_eq!(record.outcome, RecordApiUsageV3Outcome::SucceededReserved);
    assert!(above.result.unwrap().usage.is_none());
    assert_free(&funds, worst());
}

#[test]
fn a_token_count_above_the_limit_is_charged_like_a_response_without_counts() {
    let above = 9_007_199_254_740_992;
    let without_guard = guard(2 * worst());
    let without = call(
        &Fake::answering(200, r#"{"choices":[{"message":{"content":"hi"}}]}"#),
        &without_guard,
    );

    for (tokens_in, tokens_out) in [(above, 4), (3, above), (u64::MAX, 4), (3, u64::MAX)] {
        let guard = guard(2 * worst());
        let outcome = call(&answering_counts(tokens_in, tokens_out), &guard);

        assert_eq!(outcome, without, "{tokens_in} {tokens_out}");
        assert_eq!(outcome.spent_micro_usd, worst());
        let record = outcome.record.expect("a record");
        assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reserved);
        assert_eq!(record.cost_estimate_micro_usd.get(), worst());
        assert_eq!((record.tokens_in.get(), record.tokens_out.get()), (0, 0));
        assert_eq!(record.outcome, RecordApiUsageV3Outcome::SucceededReserved);
        assert_free(&guard, worst());
    }
}

#[test]
fn usage_above_the_reservation_is_charged_in_full() {
    let body = r#"{"choices":[{"message":{"content":"hi"}}],"usage":{"prompt_tokens":0,"completion_tokens":1000000}}"#;
    let guard = guard(worst());
    let outcome = call(&Fake::answering(200, body), &guard);

    assert_eq!(outcome.spent_micro_usd, 8_000_000);
    assert_eq!(
        outcome
            .record
            .expect("a record")
            .cost_estimate_micro_usd
            .get(),
        8_000_000
    );
}

struct Panicking;

impl Transport for Panicking {
    fn send(&self, _: &PreparedRequest) -> Reply {
        panic!("transport failure");
    }
}

#[test]
fn a_panic_in_the_transport_keeps_the_reservation_held() {
    let guard = guard(2 * worst());
    let unwound = catch_unwind(AssertUnwindSafe(|| call(&Panicking, &guard)));

    assert!(unwound.is_err());
    assert_eq!(guard.lock().unwrap().spent_micro_usd(), 0);
    assert_free(&guard, worst());
    assert_free_in(&guard, next_month(), 2 * worst());
}

/// Runs a whole call in the next month while the first one is open, then answers the first.
struct TurningMonth<'a> {
    guard: &'a Mutex<SpendGuard>,
    reply: Reply,
    later: Mutex<Option<CallOutcome>>,
}

impl Transport for TurningMonth<'_> {
    fn send(&self, _: &PreparedRequest) -> Reply {
        let outcome = run_call(
            &Fake::answering(200, REPORTED),
            self.guard,
            &provider(),
            &request(),
            next_month(),
            NEXT_TIMESTAMP,
        );
        *self.later.lock().unwrap() = Some(outcome);
        self.reply.clone()
    }
}

/// Sum of the records dated in `month`, as a ledger would total them.
fn ledger_total(outcomes: &[&CallOutcome], month: &str) -> u64 {
    outcomes
        .iter()
        .filter_map(|outcome| outcome.record.as_ref())
        .filter(|record| record.ts.to_string().starts_with(month))
        .map(|record| record.cost_estimate_micro_usd.get())
        .sum()
}

#[test]
fn a_call_open_across_the_month_turn_stays_in_its_own_month() {
    let replies = [
        (Fake::answering(200, REPORTED).reply, REPORTED_COST),
        (Err(TransportError::MaybeSent), worst()),
        (Err(TransportError::NotSent), 0),
    ];
    for (reply, old_cost) in replies {
        // Room for one hold only: the later call fits only if the old one stopped counting.
        let guard = guard(worst());
        let turning = TurningMonth {
            guard: &guard,
            reply,
            later: Mutex::new(None),
        };
        let first = call(&turning, &guard);
        let later = turning.later.lock().unwrap().take().expect("it ran");

        assert!(later.result.is_ok(), "{:?}", later.result);
        let outcomes = [&first, &later];
        assert_eq!(ledger_total(&outcomes, "2030-01"), old_cost);
        let current = ledger_total(&outcomes, "2030-02");
        assert_eq!(current, REPORTED_COST);
        assert_eq!(later.spent_micro_usd, current);
        assert_eq!(first.spent_micro_usd, current);
        assert_eq!(guard.lock().unwrap().spent_micro_usd(), current);
        assert_free_in(&guard, next_month(), worst() - current);
    }
}

/// Reports that it was entered, then waits to be told to answer.
struct Held {
    entered: Sender<()>,
    answer: Mutex<Receiver<()>>,
}

impl Transport for Held {
    fn send(&self, _: &PreparedRequest) -> Reply {
        self.entered.send(()).expect("test is listening");
        let told = self.answer.lock().unwrap().recv_timeout(WAIT);
        told.map_err(|_| TransportError::NotSent)?;
        Err(TransportError::MaybeSent)
    }
}

fn held() -> (Held, Receiver<()>, Sender<()>) {
    let (entered, was_entered) = channel();
    let (tell, answer) = channel();
    let answer = Mutex::new(answer);
    (Held { entered, answer }, was_entered, tell)
}

#[test]
fn open_calls_do_not_block_each_other_and_stay_under_the_cap() {
    let guard = guard(2 * worst());
    let (first, first_entered, answer_first) = held();
    let (second, second_entered, answer_second) = held();
    let third = Fake::answering(200, REPORTED);

    thread::scope(|scope| {
        let a = scope.spawn(|| call(&first, &guard));
        first_entered.recv_timeout(WAIT).expect("first is sending");
        assert!(guard.try_lock().is_ok(), "the guard is locked during send");

        let b = scope.spawn(|| call(&second, &guard));
        second_entered
            .recv_timeout(WAIT)
            .expect("second is sending");

        // Nothing is spent yet, but both reservations count against the cap.
        let refused = call(&third, &guard);
        assert_eq!(refused.result, Err(CallError::Refused(Refusal::CapReached)));
        assert_eq!(refused.spent_micro_usd, 0);
        assert_eq!(third.calls(), 0);

        answer_second.send(()).expect("second waits");
        assert_eq!(b.join().expect("second").spent_micro_usd, worst());
        answer_first.send(()).expect("first waits");
        assert_eq!(a.join().expect("first").spent_micro_usd, 2 * worst());
    });
}

/// Puts a fresh guard in the place of the one that holds the reservation.
struct Replacing<'a> {
    guard: &'a Mutex<SpendGuard>,
    reply: Reply,
}

impl Transport for Replacing<'_> {
    fn send(&self, _: &PreparedRequest) -> Reply {
        *self.guard.lock().unwrap() = spend_guard(worst());
        self.reply.clone()
    }
}

#[test]
fn a_guard_replaced_during_the_call_is_reported_with_the_record() {
    let guard = guard(worst());
    let replacing = |reply| Replacing {
        guard: &guard,
        reply,
    };
    let outcome = call(&replacing(Err(TransportError::MaybeSent)), &guard);
    assert_eq!(outcome.result, Err(CallError::GuardReplaced));
    assert_eq!(outcome.spent_micro_usd, 0);
    let record = outcome.record.expect("the caller still writes it");
    assert_eq!(record.cost_estimate_micro_usd.get(), worst());
    assert_eq!(
        record.outcome,
        RecordApiUsageV3Outcome::ChargedGuardReplaced
    );
    assert_free(&guard, worst());

    let outcome = call(&replacing(Fake::answering(200, REPORTED).reply), &guard);
    assert_eq!(outcome.result, Err(CallError::GuardReplaced));
    assert_eq!(outcome.spent_micro_usd, 0);
    let record = outcome.record.expect("the caller still writes it");
    assert_eq!(
        record.outcome,
        RecordApiUsageV3Outcome::ChargedGuardReplaced
    );
    assert_eq!(record.cost_basis, RecordApiUsageV3CostBasis::Reported);
    assert_eq!(record.cost_estimate_micro_usd.get(), REPORTED_COST);
    assert_eq!((record.tokens_in.get(), record.tokens_out.get()), (3, 4));
    assert_free(&guard, worst());

    let outcome = call(&replacing(Err(TransportError::NotSent)), &guard);
    assert_eq!(outcome.result, Err(CallError::NotSent));
    assert_eq!(outcome.record, None);
}

#[test]
fn refused_calls_keep_the_adapter_wording() {
    assert_eq!(
        CallError::Refused(Refusal::Disabled).to_string(),
        "the api adapter is disabled"
    );
    assert_eq!(
        CallError::Refused(Refusal::CapReached).to_string(),
        "the monthly cap is reached"
    );
}

#[test]
fn errors_and_debug_output_carry_no_text_body_or_key_reference() {
    let mut bad = request();
    bad.temperature = Some(9.0);
    let fakes = [
        Fake::answering(200, REPORTED),
        Fake::answering(500, REPORTED),
        Fake::answering(200, "very-private-broken-body"),
        Fake::new(Err(TransportError::MaybeSent)),
        Fake::new(Err(TransportError::NotSent)),
    ];
    let guard = guard(100 * worst());
    let mut outcomes: Vec<CallOutcome> = fakes.iter().map(|f| call(f, &guard)).collect();
    outcomes.push(run_call(
        &fakes[0],
        &guard,
        &provider(),
        &bad,
        month(),
        TIMESTAMP,
    ));

    let mut shown = String::new();
    for outcome in &outcomes {
        shown.push_str(&format!("{outcome:?}\n"));
        if let Err(error) = &outcome.result {
            shown.push_str(&format!("{error} {error:?}\n"));
        }
    }
    for fake in &fakes {
        shown.push_str(&format!("{:?}\n", fake.reply));
        if let Err(error) = &fake.reply {
            shown.push_str(&format!("{error}\n"));
        }
    }
    for hidden in ["very-private", KEY_ALIAS] {
        assert!(!shown.contains(hidden), "{hidden}");
    }
}

#[test]
fn a_billed_failure_and_a_success_without_tokens_produce_different_records() {
    let failure = call(&Fake::new(Err(TransportError::MaybeSent)), &guard(worst()));
    let success = call(
        &Fake::answering(299, r#"{"choices":[{"message":{"content":"hi"}}]}"#),
        &guard(worst()),
    );
    assert_eq!(failure.result, Err(CallError::Uncertain));
    assert!(success.result.expect("successful call").usage.is_none());

    let failed_record = failure.record.expect("failure has a record");
    let successful_record = success.record.expect("success has a record");
    assert_ne!(failed_record, successful_record);
    assert_eq!(
        failed_record.into_fields(),
        ads_contracts::RecordApiUsageV3Fields {
            outcome: RecordApiUsageV3Outcome::ChargedUncertain,
            ..successful_record.into_fields()
        },
    );
}
