use ads_adapter_api::{
    ApiConfig, ChatRequest, ConfigError, MAX_KEY_ALIAS_CHARS, MAX_KEY_ALIASES, Message, Month,
    Refusal, Role, SpendGuard, build_request, parse_response,
};

const PROVIDER: &str = r#"
[providers.provider-a]
base_url = "https://api.example.com/v1"
model = "model-x"
input_usd_per_mtok = 2.0
output_usd_per_mtok = 8.0
price_version = "prices-2030-01-01"
"#;

const TWO_KEYS: &str = r#"key_aliases = ["personal", "work"]"#;

fn load(top: &str, provider: &str) -> Result<ApiConfig, ConfigError> {
    ApiConfig::from_toml(&format!("{top}\n{PROVIDER}\n{provider}"))
}

fn with_active(active: &str) -> ApiConfig {
    load("", &format!("{TWO_KEYS}\nactive_key = \"{active}\"")).expect("valid")
}

fn reference(config: &ApiConfig) -> String {
    let provider = config.provider("provider-a").expect("provider");
    let request = ChatRequest {
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_owned(),
        }],
        max_tokens: None,
        temperature: None,
    };
    let prepared = build_request(provider, &request).expect("request");
    prepared.key.handle.reference().to_owned()
}

fn refused_field(top: &str, provider: &str) -> String {
    load(top, provider).expect_err(provider).field
}

#[test]
fn a_single_key_needs_no_configuration() {
    let config = load("", "").expect("valid");
    assert_eq!(reference(&config), "provider-a/default");
    let keys = &config.provider("provider-a").unwrap().keys;
    assert_eq!(keys.iter().collect::<Vec<_>>(), ["default"]);

    let named = load("", r#"key_aliases = ["work"]"#).expect("one needs no active_key");
    assert_eq!(reference(&named), "provider-a/work");
}

#[test]
fn a_provider_list_wins_over_the_general_one() {
    let top = r#"key_aliases = ["general"]"#;
    assert_eq!(
        reference(&load(top, "").expect("inherited")),
        "provider-a/general"
    );
    let own = load(top, r#"key_aliases = ["own"]"#).expect("own");
    assert_eq!(reference(&own), "provider-a/own");
}

#[test]
fn the_same_nickname_gives_each_provider_its_own_reference() {
    let second = PROVIDER.replace("provider-a", "provider-b");
    let config = load("", &second).expect("valid");
    let b = config.provider("provider-b").expect("b");
    assert_eq!(b.key_handle().reference(), "provider-b/default");
    assert_eq!(reference(&config), "provider-a/default");
}

#[test]
fn the_active_key_must_be_in_the_list() {
    let outside = format!("{TWO_KEYS}\nactive_key = \"other\"");
    assert_eq!(refused_field("", &outside), "providers[1].active_key");
    assert_eq!(
        refused_field("", r#"active_key = "work""#),
        "providers[1].active_key"
    );
    assert!(load("", r#"active_key = "default""#).is_ok());
}

#[test]
fn several_keys_need_an_explicit_choice() {
    assert_eq!(refused_field("", TWO_KEYS), "providers[1].active_key");
    assert_eq!(refused_field(TWO_KEYS, ""), "providers[1].active_key");
}

#[test]
fn a_repeated_nickname_is_refused() {
    let own = "key_aliases = [\"work\", \"work\"]\nactive_key = \"work\"";
    assert_eq!(refused_field("", own), "providers[1].key_aliases[1]");
    // The general list is checked even when no provider inherits it.
    let general = ApiConfig::from_toml(r#"key_aliases = ["work", "work"]"#);
    assert_eq!(general.expect_err("repeated").field, "key_aliases[1]");
}

#[test]
fn the_list_size_and_the_nickname_format_are_limited() {
    let list = |names: &[String]| format!("key_aliases = {names:?}\nactive_key = \"{}\"", names[0]);
    let many = |n: usize| (0..n).map(|i| format!("k{i}")).collect::<Vec<_>>();
    assert!(load("", &list(&many(MAX_KEY_ALIASES))).is_ok());
    assert!(load("", &list(&many(MAX_KEY_ALIASES + 1))).is_err());
    assert!(load("", "key_aliases = []").is_err());

    for good in ["a", "work_2", "team-b", &"a".repeat(MAX_KEY_ALIAS_CHARS)] {
        assert!(load("", &list(&[good.to_owned()])).is_ok(), "{good}");
    }
    for bad in [
        "",
        "Work",
        "2work",
        "-work",
        "my key",
        "a/b",
        "a.b",
        "a:b",
        "a@b",
        "chave-é",
        &"a".repeat(MAX_KEY_ALIAS_CHARS + 1),
    ] {
        assert!(load("", &list(&[bad.to_owned()])).is_err(), "{bad}");
    }
}

#[test]
fn switching_by_hand_changes_the_reference() {
    assert_eq!(reference(&with_active("personal")), "provider-a/personal");
    assert_eq!(reference(&with_active("work")), "provider-a/work");
}

#[test]
fn calls_and_failures_never_rotate_the_key() {
    let config = with_active("work");
    let capped = load(
        "enabled = true\nmonthly_cap_usd = 1.0",
        &format!("{TWO_KEYS}\nactive_key = \"work\""),
    )
    .expect("valid");
    let mut guard = SpendGuard::new(&capped, Month::new(2030, 1).unwrap(), 0);
    for _ in 0..4 {
        assert_eq!(reference(&config), "provider-a/work");
        let held = guard
            .authorize(Month::new(2030, 1).unwrap(), 10)
            .expect("fits");
        assert!(parse_response(r#"{"error":{"code":"invalid_api_key"}}"#).is_err());
        guard.release(held).expect("own");
        assert_eq!(reference(&capped), "provider-a/work");
    }
}

#[test]
fn switching_the_key_neither_resets_nor_raises_the_cap() {
    let month = Month::new(2030, 1).unwrap();
    let capped = |active: &str| {
        load(
            "enabled = true\nmonthly_cap_usd = 1.0",
            &format!("{TWO_KEYS}\nactive_key = \"{active}\""),
        )
        .expect("valid")
    };
    let before = capped("personal");
    let after = capped("work");
    assert_eq!(before.monthly_cap_micro_usd, after.monthly_cap_micro_usd);

    // A guard rebuilt from the switched configuration starts from the same ledger total.
    let mut rebuilt = SpendGuard::new(&after, month, 900_000);
    assert_eq!(
        rebuilt.authorize(month, 100_001).unwrap_err(),
        Refusal::WouldExceedCap
    );
    let last = rebuilt.authorize(month, 100_000).expect("the remainder");
    rebuilt.settle(last, 100_000).expect("own");
    assert_eq!(
        rebuilt.authorize(month, 0).unwrap_err(),
        Refusal::CapReached
    );

    // A cap per key does not exist.
    assert!(load("", "monthly_cap_usd = 5.0").is_err());
}

#[test]
fn errors_never_repeat_a_pasted_secret() {
    // Made-up values, shaped like keys.
    for secret in [
        "sk-test-000",
        "SK-Test-000",
        "gsk_test000",
        "0123456789abcdef0123",
        "abcd-0123456789abcdef",
        "Zx9Qm2Lp7Rt4Vw8Ny3Kb6Hd1Fs5Gj0AaCcEe",
        "q7!pasted secret/value+==",
    ] {
        let quoted = format!("{secret:?}");
        for provider in [
            format!("key_aliases = [{quoted}]"),
            format!("key_aliases = [\"work\", {quoted}]\nactive_key = \"work\""),
            format!("active_key = {quoted}"),
            format!("key_aliases = {quoted}"),
            format!("key_aliases = {{ work = {quoted} }}"),
            format!("key_aliases = [[{quoted}]]"),
            format!("active_key = [{quoted}]"),
            // Unquoted, so the file itself does not parse.
            format!("active_key = {secret}"),
            format!("key_aliases = [{secret}]"),
            format!("active_key = \"{secret}"),
        ] {
            let error = load("", &provider).expect_err(&provider);
            let shown = format!("{error} {error:?}");
            assert!(!shown.contains(secret), "{provider}: {shown}");
        }
        for top in [
            format!("key_aliases = {quoted}"),
            format!("key_aliases = [{quoted}]"),
        ] {
            let error = load(&top, "").expect_err(&top);
            assert!(!format!("{error} {error:?}").contains(secret), "{top}");
        }
    }
}

#[test]
fn text_that_looks_like_a_secret_is_refused_even_in_a_valid_format() {
    for secret in ["sk-test", "hf_test", "0123456789abcdef", "a1b2c3d4e5f6a7b8"] {
        let error = load("", &format!("key_aliases = [\"{secret}\"]")).expect_err(secret);
        assert!(error.reason.contains("looks like a secret"), "{secret}");
    }
    for name in ["desk-key", "personal-account-2030", "a1b2c3d4e5f6a7b"] {
        assert!(
            load("", &format!("key_aliases = [\"{name}\"]")).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn debug_output_hides_every_key_reference() {
    let config = load(
        r#"key_aliases = ["zebra"]"#,
        "key_aliases = [\"otter\", \"heron\"]\nactive_key = \"heron\"",
    )
    .expect("valid");
    let provider = config.provider("provider-a").expect("provider");
    let shown = format!(
        "{config:?} {provider:?} {:?} {:?}",
        provider.keys,
        provider.key_handle()
    );
    for hidden in ["zebra", "otter", "heron"] {
        assert!(!shown.contains(hidden), "{hidden}");
    }
}
