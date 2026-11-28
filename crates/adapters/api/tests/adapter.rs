use ads_adapter_api::{
    ApiConfig, ChatRequest, MAX_TOKEN_COUNT, MapError, Message, Month, Refusal, Role, SpendGuard,
    TokenUsage, build_request, parse_request_body, parse_response, usage_record,
};
use ads_contracts::{RecordApiUsageV3CostBasis, RecordApiUsageV3Outcome};

const PROVIDER: &str = r#"
[providers.provider-a]
base_url = "https://api.example.com/v1/"
model = "model-x"
input_usd_per_mtok = 2.0
output_usd_per_mtok = 8.0
price_version = "prices-2030-01-01"
"#;

fn config(extra: &str) -> ApiConfig {
    ApiConfig::from_toml(&format!("{extra}\n{PROVIDER}")).expect("valid config")
}

fn month(m: u8) -> Month {
    Month::new(2030, m).expect("month")
}

fn request(text: &str) -> ChatRequest {
    ChatRequest {
        messages: vec![
            Message {
                role: Role::System,
                content: "be brief".to_owned(),
            },
            Message {
                role: Role::User,
                content: text.to_owned(),
            },
        ],
        max_tokens: Some(100),
        temperature: Some(0.5),
    }
}

#[test]
fn defaults_are_off_without_cap_or_provider() {
    let defaults = ApiConfig::defaults();
    assert!(!defaults.enabled());
    assert_eq!(defaults.monthly_cap_micro_usd(), None);
    assert_eq!(defaults.providers().count(), 0);
}

#[test]
fn a_provider_with_a_cap_is_still_refused_while_disabled() {
    let cfg = config("monthly_cap_usd = 10.0");
    assert!(!cfg.enabled());
    let mut guard = SpendGuard::new(&cfg, month(1), 0);
    assert_eq!(guard.authorize(month(1), 1).unwrap_err(), Refusal::Disabled);
}

#[test]
fn enabled_without_a_cap_refuses_every_call() {
    let cfg = config("enabled = true");
    let mut guard = SpendGuard::new(&cfg, month(1), 0);
    assert_eq!(guard.authorize(month(1), 0).unwrap_err(), Refusal::NoCap);
}

#[test]
fn the_guard_stops_at_the_cap_and_reserves_the_worst_case() {
    let cfg = config("enabled = true\nmonthly_cap_usd = 1.0");
    let mut guard = SpendGuard::new(&cfg, month(1), 0);
    assert_eq!(
        guard.authorize(month(1), 1_000_001).unwrap_err(),
        Refusal::WouldExceedCap
    );
    let first = guard.authorize(month(1), 600_000).expect("fits");
    // 600k held: another 600k would pass the cap even though nothing is spent yet.
    assert_eq!(
        guard.authorize(month(1), 600_000).unwrap_err(),
        Refusal::WouldExceedCap
    );
    guard.settle(first, 1_000_000).expect("own");
    assert_eq!(guard.spent_micro_usd(), 1_000_000);
    assert_eq!(
        guard.authorize(month(1), 0).unwrap_err(),
        Refusal::CapReached
    );
}

#[test]
fn an_overrun_still_counts_and_a_release_charges_nothing() {
    let cfg = config("enabled = true\nmonthly_cap_usd = 1.0");
    let mut guard = SpendGuard::new(&cfg, month(1), 0);
    let held = guard.authorize(month(1), 500_000).expect("fits");
    guard.release(held).expect("own");
    assert_eq!(guard.spent_micro_usd(), 0);
    let held = guard.authorize(month(1), 500_000).expect("fits");
    guard.settle(held, 900_000).expect("own");
    assert_eq!(guard.spent_micro_usd(), 900_000);
}

#[test]
fn a_new_month_resets_but_a_clock_set_back_does_not() {
    let cfg = config("enabled = true\nmonthly_cap_usd = 1.0");
    let mut guard = SpendGuard::new(&cfg, month(3), 1_000_000);
    assert_eq!(
        guard.authorize(month(3), 0).unwrap_err(),
        Refusal::CapReached
    );
    assert_eq!(
        guard.authorize(month(2), 0).unwrap_err(),
        Refusal::CapReached
    );
    assert!(guard.authorize(month(4), 10).is_ok());
}

#[test]
fn finishing_an_old_reservation_leaves_the_new_month_untouched() {
    let cfg = config("enabled = true\nmonthly_cap_usd = 1.0");
    for settle in [true, false] {
        let mut guard = SpendGuard::new(&cfg, month(1), 100_000);
        let old = guard.authorize(month(1), 600_000).expect("fits");
        // The old hold does not count in the new month.
        let new = guard.authorize(month(2), 700_000).expect("fits");
        if settle {
            guard.settle(old, 600_000).expect("own");
        } else {
            guard.release(old).expect("own");
        }
        assert_eq!(guard.spent_micro_usd(), 0);
        // Only the new hold is committed: finishing the old one frees none of it.
        assert_eq!(
            guard.authorize(month(2), 300_001).unwrap_err(),
            Refusal::WouldExceedCap
        );
        let rest = guard.authorize(month(2), 300_000).expect("the remainder");
        guard.release(rest).expect("own");
        guard.settle(new, 200_000).expect("own");
        assert_eq!(guard.spent_micro_usd(), 200_000);
    }
}

#[test]
fn a_reservation_made_with_the_clock_set_back_counts_in_the_guard_month() {
    let cfg = config("enabled = true\nmonthly_cap_usd = 1.0");
    let mut guard = SpendGuard::new(&cfg, month(3), 0);
    let held = guard.authorize(month(2), 600_000).expect("fits");
    assert_eq!(
        guard.authorize(month(3), 400_001).unwrap_err(),
        Refusal::WouldExceedCap
    );
    guard.settle(held, 600_000).expect("own");
    assert_eq!(guard.spent_micro_usd(), 600_000);
    for now in [month(2), month(3)] {
        assert_eq!(
            guard.authorize(now, 400_001).unwrap_err(),
            Refusal::WouldExceedCap
        );
    }
}

#[test]
fn monthly_cap_preserves_exact_micro_dollars() {
    let cfg = config("enabled = true\nmonthly_cap_usd = 2.01");
    assert_eq!(cfg.monthly_cap_micro_usd(), Some(2_010_000));
    let mut guard = SpendGuard::new(&cfg, month(1), 0);
    let reservation = guard.authorize(month(1), 2_010_000).expect("fits");
    assert_eq!(
        guard.authorize(month(1), 1).unwrap_err(),
        Refusal::CapReached
    );
    guard.release(reservation).expect("own");
    for (value, micro_usd) in [
        ("2_010e-3", 2_010_000),
        ("1", 1_000_000),
        ("1e9", 1_000_000_000_000_000),
    ] {
        let cfg = config(&format!("monthly_cap_usd = {value}"));
        assert_eq!(cfg.monthly_cap_micro_usd(), Some(micro_usd));
    }
}

#[test]
fn monthly_cap_above_the_maximum_has_its_own_error() {
    for value in ["1000000000.000001", "1000000000.0000001", "1e30"] {
        let error = ApiConfig::from_toml(&format!("monthly_cap_usd = {value}")).unwrap_err();
        assert_eq!(error.field, "monthly_cap_usd");
        assert_eq!(error.reason, "must be at most one billion USD", "{value}");
    }
    for (value, reason) in [
        ("0.0", "must be greater than zero"),
        ("-1.0", "must be greater than zero"),
        ("1e-7", "below one micro-dollar"),
        ("2.0100001", "must be in whole micro-dollars"),
    ] {
        let error = ApiConfig::from_toml(&format!("monthly_cap_usd = {value}")).unwrap_err();
        assert_eq!(error.field, "monthly_cap_usd");
        assert_eq!(error.reason, reason, "{value}");
    }
}

#[test]
fn config_refuses_bad_values() {
    for bad in [
        "monthly_cap_usd = 0.0",
        "monthly_cap_usd = -1.0",
        "key_header = \"bad header\"",
        "chat_path = \"no-slash\"",
        "max_output_tokens = 0",
        "unknown_key = 1",
    ] {
        assert!(ApiConfig::from_toml(bad).is_err(), "{bad}");
    }
    for bad_url in [
        "http://api.example.com",
        "https://user:pw@api.example.com",
        "https://api.example.com/v1?x=1",
        "https://",
    ] {
        let toml = PROVIDER.replace("https://api.example.com/v1/", bad_url);
        assert!(ApiConfig::from_toml(&toml).is_err(), "{bad_url}");
    }
    let email_alias = PROVIDER.replace("provider-a", "user@example.com");
    assert!(ApiConfig::from_toml(&email_alias).is_err());
}

#[test]
fn invalid_provider_names_stay_out_of_diagnostics() {
    let sentinel = "synthetic@invalid";
    let source = PROVIDER.replace("provider-a", &format!("{sentinel:?}"));
    for input in [source.clone(), format!("{source}\n{source}")] {
        let error = ApiConfig::from_toml(&input).expect_err("invalid provider");
        assert!(!error.to_string().contains(sentinel));
        assert!(!format!("{error:?}").contains(sentinel));
    }
}

#[test]
fn config_diagnostics_locate_toml_syntax_errors() {
    let source = "# café\r\nenabled = true\r\nsynthetic-field = \"synthetic-value\n";
    assert_config_diagnostic(source, "line 3");
}

#[test]
fn config_diagnostics_locate_invalid_global_settings() {
    for (source, diagnostic) in [
        ("enabled = 'synthetic-value'", "enabled: must be a boolean"),
        (
            "[enabled]\nsynthetic-field = 'synthetic-value'",
            "enabled: must be a boolean",
        ),
        (
            "providers = 'synthetic-value'",
            "providers: must be a table",
        ),
        (
            "chat_path = ['synthetic-value']",
            "chat_path: must be a path starting with /",
        ),
        (
            "max_output_tokens = -1",
            "max_output_tokens: must be an integer from 1 to 4294967295",
        ),
        (
            "max_request_bytes = 0",
            "max_request_bytes: must be a positive integer in bytes",
        ),
        (
            "key_aliases = { synthetic-field = 'synthetic-value' }",
            "key_aliases: must be a list of key aliases",
        ),
    ] {
        assert_config_diagnostic(source, diagnostic);
    }
}

#[test]
fn config_diagnostics_locate_unknown_keys_without_echoing_input() {
    let source = "# café\r\nenabled = true\r\n'z-synthetic-field' = '''\r\nsynthetic-value'''\r\na-synthetic-field = 1";
    assert_config_diagnostic(source, "config: unknown field at line 3");

    let source = "# café\n\n[synthetic-field]\nvalue = 'synthetic-value'";
    assert_config_diagnostic(source, "config: unknown field at line 3");

    let provider = PROVIDER.replace("provider-a", "synthetic-provider");
    let provider = provider.trim_end();
    let source = format!("{provider}\nsynthetic-field = [\n'synthetic-value'\n]");
    let line = provider.lines().count() + 1;
    assert_config_diagnostic(
        &source,
        &format!("providers[1]: unknown field at line {line}"),
    );
}

#[test]
fn config_diagnostics_locate_provider_errors_in_source_order() {
    let first = PROVIDER.replace("provider-a", "z-first");
    let second = PROVIDER.replace("provider-a", "synthetic-provider");
    for (entry, location) in [
        (
            second.replace("\"model-x\"", "['synthetic-value']"),
            "providers[2].model: must be non-empty text",
        ),
        (
            second.replace("model = \"model-x\"", ""),
            "providers[2].model: missing field",
        ),
        (
            second.replace("https://api.example.com/v1/", "synthetic-value"),
            "providers[2].base_url",
        ),
        (
            format!("{second}\nmax_request_bytes = 'synthetic-value'"),
            "providers[2].max_request_bytes: must be a positive integer in bytes",
        ),
        (
            second.replace(
                "input_usd_per_mtok = 2.0",
                "input_usd_per_mtok = 'synthetic-value'",
            ),
            "providers[2].input_usd_per_mtok: must be a non-negative price",
        ),
        (
            second.replace("synthetic-provider", "\"synthetic-provider@invalid\""),
            "providers[2]:",
        ),
    ] {
        assert_config_diagnostic(&format!("{first}\n{entry}"), location);
    }
    assert_config_diagnostic(
        "[providers]\nsynthetic-provider = 'synthetic-value'",
        "providers[1]:",
    );
}

fn assert_config_diagnostic(source: &str, location: &str) {
    let error = ApiConfig::from_toml(source).expect_err("invalid configuration");
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(!rendered.contains("synthetic-provider"));
        assert!(!rendered.contains("synthetic-field"));
        assert!(!rendered.contains("synthetic-value"));
    }
    assert!(error.to_string().contains(location), "{error}");
}

#[test]
fn secret_like_provider_names_cannot_reach_usage_records() {
    for name in ["SK-synthetic", "synthetic123456789", "Lab.A:west_2"] {
        let source = PROVIDER.replace("provider-a", &format!("{name:?}"));
        match ApiConfig::from_toml(&source) {
            Ok(config) => {
                let record = usage_record(
                    config.provider(name).expect("configured alias"),
                    "2030-01-02T03:04:05Z",
                    None,
                    1,
                    RecordApiUsageV3Outcome::SucceededReserved,
                )
                .expect("valid record");
                let json = serde_json::to_value(record).unwrap();
                assert_eq!(json["provider_alias"], "Lab.A:west_2");
            }
            Err(error) => {
                assert_ne!(name, "Lab.A:west_2");
                assert_eq!(error.field, "providers[1]");
                assert!(!error.to_string().contains(name));
                assert!(!format!("{error:?}").contains(name));
            }
        }
    }
}

#[test]
fn provider_overrides_win_over_the_defaults() {
    let toml = format!(
        "{PROVIDER}\nkey_header = \"x-api-key\"\nkey_prefix = \"\"\n\n[providers.provider-b]\nbase_url = \"https://b.example.com\"\nmodel = \"model-y\"\ninput_usd_per_mtok = 1.0\noutput_usd_per_mtok = 1.0\nprice_version = \"p\"\nchat_path = \"/v2/chat\"\nkey_header = \"authorization\"\nmax_output_tokens = 50\n"
    );
    let cfg = ApiConfig::from_toml(&toml).expect("valid");
    let a = cfg.provider("provider-a").expect("a");
    let b = cfg.provider("provider-b").expect("b");
    assert_eq!(a.key_header, "x-api-key");
    assert_eq!(a.url, "https://api.example.com/v1/chat/completions");
    assert_eq!(b.url, "https://b.example.com/v2/chat");
    assert_eq!(
        (b.key_header.as_str(), b.max_output_tokens),
        ("authorization", 50)
    );
}

#[test]
fn request_size_limit_is_inclusive_and_can_be_overridden_per_provider() {
    let input = request("text with a newline\n");
    let defaults = config("");
    let provider = defaults.provider("provider-a").unwrap();
    let bytes = build_request(provider, &input).unwrap().body.len();

    let limited = config(&format!("max_request_bytes = {bytes}"));
    let provider = limited.provider("provider-a").unwrap();
    assert_eq!(build_request(provider, &input).unwrap().body.len(), bytes);

    let lower = bytes - 1;
    let toml = format!("max_request_bytes = {bytes}\n{PROVIDER}\nmax_request_bytes = {lower}");
    let limited = ApiConfig::from_toml(&toml).unwrap();
    let provider = limited.provider("provider-a").unwrap();
    assert_eq!(
        build_request(provider, &input),
        Err(MapError::RequestTooLarge)
    );

    let toml = format!("{PROVIDER}\nmax_request_bytes = 0");
    assert_eq!(
        ApiConfig::from_toml(&toml).unwrap_err().field,
        "providers[1].max_request_bytes"
    );
}

#[test]
fn the_request_maps_to_the_wire_format_and_back() {
    let cfg = config("");
    let provider = cfg.provider("provider-a").expect("provider");
    let original = request("hello");
    let prepared = build_request(provider, &original).expect("ok");

    let wire: serde_json::Value = serde_json::from_str(&prepared.body).expect("json");
    assert_eq!(wire["model"], "model-x");
    assert_eq!(wire["stream"], false);
    assert_eq!(wire["messages"][1]["role"], "user");
    assert_eq!(prepared.url, "https://api.example.com/v1/chat/completions");
    assert_eq!(prepared.key.header, "Authorization");
    assert_eq!(prepared.key.prefix, "Bearer ");

    let (model, parsed) = parse_request_body(&prepared.body).expect("round trip");
    assert_eq!(model, "model-x");
    assert_eq!(parsed, original);
}

#[test]
fn defaults_fill_max_tokens_and_limits_are_enforced() {
    let cfg = config("max_output_tokens = 64");
    let provider = cfg.provider("provider-a").expect("provider");
    let mut req = request("hi");
    req.max_tokens = None;
    let prepared = build_request(provider, &req).expect("ok");
    assert_eq!(
        parse_request_body(&prepared.body).unwrap().1.max_tokens,
        Some(64)
    );

    req.max_tokens = Some(65);
    assert_eq!(
        build_request(provider, &req).unwrap_err(),
        MapError::MaxTokens
    );
    req.max_tokens = Some(0);
    assert_eq!(
        build_request(provider, &req).unwrap_err(),
        MapError::MaxTokens
    );
    req.max_tokens = Some(10);
    req.temperature = Some(3.0);
    assert_eq!(
        build_request(provider, &req).unwrap_err(),
        MapError::Temperature
    );
    req.temperature = None;
    req.messages.clear();
    assert_eq!(
        build_request(provider, &req).unwrap_err(),
        MapError::EmptyMessages
    );
}

#[test]
fn worst_case_counts_body_bytes_as_input_and_max_tokens_as_output() {
    let cfg = config("");
    let provider = cfg.provider("provider-a").expect("provider");
    let prepared = build_request(provider, &request("hello")).unwrap();
    let expected = provider.cost_micro_usd(prepared.body.len() as u64, 100);
    assert_eq!(prepared.worst_case_micro_usd, expected);
    // 100 output tokens at 8 USD per million is 800 micro-dollars, plus the input bound.
    assert!(prepared.worst_case_micro_usd >= 800);
}

#[test]
fn the_response_maps_text_and_usage() {
    let body = r#"{"model":"model-x","choices":[{"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
    let response = parse_response(body).expect("ok");
    assert_eq!(response.text, "hi");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    assert_eq!(
        response.usage,
        Some(TokenUsage {
            tokens_in: 10,
            tokens_out: 5
        })
    );
}

#[test]
fn a_response_without_complete_usage_has_none_and_a_malformed_one_is_refused() {
    let partial = r#"{"choices":[{"message":{"content":null}}],"usage":{"prompt_tokens":10}}"#;
    let response = parse_response(partial).expect("ok");
    assert_eq!((response.text.as_str(), response.usage), ("", None));
    for bad in ["{}", r#"{"choices":[]}"#, "not json", r#"{"choices":[{}]}"#] {
        assert_eq!(
            parse_response(bad).unwrap_err(),
            MapError::MalformedResponse,
            "{bad}"
        );
    }
}

#[test]
fn a_token_count_above_the_limit_leaves_the_usage_unreported() {
    const LIMIT: u64 = 9_007_199_254_740_991;
    assert_eq!(MAX_TOKEN_COUNT, LIMIT);
    let parse = |tokens_in: u64, tokens_out: u64| {
        let body = format!(
            r#"{{"model":"model-x","choices":[{{"message":{{"content":"hi"}},"finish_reason":"stop"}}],"usage":{{"prompt_tokens":{tokens_in},"completion_tokens":{tokens_out}}}}}"#
        );
        parse_response(&body).expect("ok")
    };

    let at_limit = parse(LIMIT, LIMIT);
    assert_eq!(
        at_limit.usage,
        Some(TokenUsage {
            tokens_in: LIMIT,
            tokens_out: LIMIT
        })
    );
    for (tokens_in, tokens_out) in [
        (LIMIT + 1, 4),
        (3, LIMIT + 1),
        (u64::MAX, 4),
        (3, u64::MAX),
        (u64::MAX, u64::MAX),
    ] {
        let mut expected = at_limit.clone();
        expected.usage = None;
        assert_eq!(
            parse(tokens_in, tokens_out),
            expected,
            "{tokens_in} {tokens_out}"
        );
    }

    let beyond_u64 = r#"{"choices":[{"message":{"content":"hi"}}],"usage":{"prompt_tokens":18446744073709551616,"completion_tokens":4}}"#;
    assert_eq!(
        parse_response(beyond_u64).unwrap_err(),
        MapError::MalformedResponse
    );
}

#[test]
fn the_usage_record_is_sized_from_reported_tokens_or_the_reserve() {
    let cfg = config("");
    let provider = cfg.provider("provider-a").expect("provider");
    let reported = usage_record(
        provider,
        "2030-01-02T03:04:05Z",
        Some(TokenUsage {
            tokens_in: 1_000_000,
            tokens_out: 1_000_000,
        }),
        7,
        RecordApiUsageV3Outcome::SucceededReported,
    )
    .expect("record");
    assert_eq!(reported.cost_estimate_micro_usd.get(), 10_000_000);
    assert_eq!(reported.provider_alias.as_str(), "provider-a");

    let reserved = usage_record(
        provider,
        "2030-01-02T03:04:05Z",
        None,
        1234,
        RecordApiUsageV3Outcome::SucceededReserved,
    )
    .expect("record");
    assert_eq!(reserved.cost_estimate_micro_usd.get(), 1234);
    assert_eq!(
        (reserved.tokens_in.get(), reserved.tokens_out.get()),
        (0, 0)
    );
    assert!(
        usage_record(
            provider,
            "yesterday",
            None,
            0,
            RecordApiUsageV3Outcome::SucceededReserved,
        )
        .is_err()
    );
}

#[test]
fn the_usage_record_requires_counts_that_match_the_outcome() {
    use RecordApiUsageV3Outcome::*;

    let cfg = config("");
    let provider = cfg.provider("provider-a").expect("provider");
    let counts = TokenUsage {
        tokens_in: 0,
        tokens_out: 0,
    };
    for (outcome, permits_reported, permits_reserved) in [
        (SucceededReported, true, false),
        (SucceededReserved, false, true),
        (ChargedUncertain, false, true),
        (ChargedErrorStatus, false, true),
        (ChargedBadResponse, false, true),
        (ChargedGuardReplaced, true, true),
    ] {
        for (usage, expected_basis, permitted) in [
            (
                Some(counts),
                RecordApiUsageV3CostBasis::Reported,
                permits_reported,
            ),
            (None, RecordApiUsageV3CostBasis::Reserved, permits_reserved),
        ] {
            let result = usage_record(provider, "2030-01-02T03:04:05Z", usage, 7, outcome);
            if permitted {
                let record = result.expect("valid usage and outcome");
                assert_eq!(record.cost_basis, expected_basis);
                assert_eq!(record.outcome, outcome);
            } else {
                assert!(result.is_err(), "{outcome:?} with {expected_basis:?}");
            }
        }
    }
}

#[test]
fn the_usage_record_has_no_content_and_validates_against_the_contract_shape() {
    let cfg = config("");
    let provider = cfg.provider("provider-a").expect("provider");
    let secret_text = "very-private-prompt-text";
    let prepared = build_request(provider, &request(secret_text)).unwrap();
    let body = format!(
        r#"{{"choices":[{{"message":{{"content":"{secret_text}"}}}}],"usage":{{"prompt_tokens":3,"completion_tokens":4}}}}"#
    );
    let response = parse_response(&body).unwrap();
    let record = usage_record(
        provider,
        "2030-01-02T03:04:05Z",
        response.usage,
        prepared.worst_case_micro_usd,
        RecordApiUsageV3Outcome::SucceededReported,
    )
    .unwrap();

    let json = serde_json::to_string(&record).expect("json");
    assert!(!json.contains(secret_text));
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["version"], 3);
    assert_eq!(value["outcome"], "succeeded_reported");
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "cost_basis",
            "cost_estimate_micro_usd",
            "model",
            "outcome",
            "price_version",
            "provider_alias",
            "tokens_in",
            "tokens_out",
            "ts",
            "version"
        ]
    );
}

#[test]
fn debug_output_hides_prompt_text_response_text_and_the_key_reference() {
    let cfg = config("key_aliases = [\"vault-entry\"]");
    let provider = cfg.provider("provider-a").expect("provider");
    let prepared = build_request(provider, &request("very-private-prompt-text")).unwrap();
    let response =
        parse_response(r#"{"choices":[{"message":{"content":"very-private-answer"}}]}"#).unwrap();
    let shown = format!(
        "{prepared:?} {:?} {response:?} {:?} {:?} {cfg:?}",
        request("very-private-prompt-text"),
        prepared.key,
        provider.key_handle()
    );
    for hidden in [
        "very-private-prompt-text",
        "very-private-answer",
        "vault-entry",
    ] {
        assert!(!shown.contains(hidden), "{hidden}");
    }
}

#[test]
fn a_reservation_from_another_guard_is_refused_and_changes_nothing() {
    let cfg = config(
        "enabled = true
monthly_cap_usd = 0.0001",
    );
    let mut a = SpendGuard::new(&cfg, month(1), 0);
    let mut b = SpendGuard::new(&cfg, month(1), 0);
    let held_a = a.authorize(month(1), 100).expect("fits");
    let held_b = b.authorize(month(1), 100).expect("fits");
    let returned = b.release(held_a).unwrap_err().into_reservation();
    // B still holds its own 100: its counters were not reduced by A's reservation.
    assert_eq!(b.authorize(month(1), 1).unwrap_err(), Refusal::CapReached);
    b.settle(held_b, 100).expect("own");
    assert_eq!(b.spent_micro_usd(), 100);
    // The refusal handed A's reservation back: finishing it on A frees A's hold.
    assert_eq!(returned.micro_usd(), 100);
    a.release(returned).expect("issuer");
    let again = a.authorize(month(1), 100).expect("a's 100 was released");
    assert_eq!(again.micro_usd(), 100);
}

#[test]
fn tariff_precision_requires_whole_micro_dollars_per_million_tokens() {
    for (field, old) in [
        ("input_usd_per_mtok", "2.0"),
        ("output_usd_per_mtok", "8.0"),
    ] {
        let load = |price: &str| {
            ApiConfig::from_toml(
                &PROVIDER.replace(&format!("{field} = {old}"), &format!("{field} = {price}")),
            )
        };
        for price in [
            "1.49e-6",
            "0.0000004",
            "999999999.00000101",
            "1e-400",
            "-0.01",
            "nan",
            "inf",
            "1_000_000_000.000001",
        ] {
            assert_eq!(
                load(price).expect_err(price).field,
                format!("providers[1].{field}")
            );
        }
        for (price, micro) in [
            ("0.0", 0),
            ("1.00e-6", 1),
            ("0.000_249", 249),
            ("10e-7", 1),
            ("999999999.000001", 999_999_999_000_001),
            ("1e9", 1_000_000_000_000_000),
        ] {
            let cfg = load(price).expect(price);
            let p = cfg.provider("provider-a").unwrap();
            let rate = if field == "input_usd_per_mtok" {
                p.input_micro_usd_per_mtok
            } else {
                p.output_micro_usd_per_mtok
            };
            assert_eq!(rate, micro);
        }
    }
}
