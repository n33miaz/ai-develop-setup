use ads_daemon::api::ApiConfig;
use ads_daemon::auth::Token;

#[test]
fn configuration_rejects_invalid_bounds_and_origins() {
    for config in [
        "max_sessions = 0",
        "max_pending_auth = 0",
        "max_pending_auth = 1048577",
        "accept_retry_ms = 0",
        "accept_retry_ms = 300001",
        "max_write_bytes = 65537",
        "auth_timeout_ms = 0",
        "events_per_stream = 0",
        "idempotency_keys = 0",
        "unknown = 1",
        "allowed_origins = ['http://*.invalid']",
        "allowed_origins = ['null']",
        "allowed_origins = ['http://localhost:5173/path']",
    ] {
        assert!(
            ApiConfig::from_toml_str(config).is_err(),
            "invalid config accepted"
        );
    }
}

#[test]
fn interface_origin_is_derived_and_overridable() {
    let defaults = ApiConfig::from_toml_str("").unwrap();
    assert_eq!(defaults.allowed_origins, ["http://127.0.0.1:5173"]);
    let custom = ApiConfig::from_toml_str("ui_base_url = 'http://localhost:8271/'").unwrap();
    assert_eq!(custom.allowed_origins, ["http://localhost:8271"]);
    let url = Token::generate()
        .unwrap()
        .launch_url(&custom.ui_base_url, 42);
    assert!(url.starts_with("http://localhost:8271/#core=ws://127.0.0.1:42&token="));
    let explicit = ApiConfig::from_toml_str(
        "ui_base_url = 'http://localhost:8271'\nallowed_origins = ['https://localhost:9000']",
    )
    .unwrap();
    assert_eq!(explicit.allowed_origins, ["https://localhost:9000"]);
    for host in ["localhost", "127.0.0.1", "[::1]"] {
        assert!(ApiConfig::from_toml_str(&format!("ui_base_url = 'http://{host}:5173'")).is_ok());
    }
    for value in [
        "https://localhost:5173",
        "http://192.0.2.1:5173",
        "http://localhost.example:5173",
        "http://127.0.0.2:5173",
        "http://[::2]:5173",
        "null",
        "http://localhost:5173/path",
        "http://localhost:5173/#token=x",
        "http://user@localhost:5173",
        "http://*.invalid",
        "http://localhost.:5173",
        "http://LOCALHOST:5173",
        "http://127.1:5173",
        "http://0.0.0.0:5173",
        "http://[::ffff:127.0.0.1]:5173",
    ] {
        assert!(
            ApiConfig::from_toml_str(&format!("ui_base_url = '{value}'")).is_err(),
            "{value}"
        );
    }
    for value in ["http://127.0.0.1", "http://127.0.0.1:5173/"] {
        assert!(ApiConfig::from_toml_str(&format!("ui_base_url = '{value}'")).is_ok());
    }
    // The browser always sends the origin in lower case. An explicit valid list isolates each check.
    for value in ["HTTP://localhost:5173", "Http://127.0.0.1:5173"] {
        assert!(
            ApiConfig::from_toml_str(&format!(
                "ui_base_url = '{value}'\nallowed_origins = ['http://localhost:5173']"
            ))
            .is_err(),
            "{value}"
        );
    }
    assert!(
        ApiConfig::from_toml_str(
            "ui_base_url = 'http://localhost:5173'\nallowed_origins = ['HTTPS://localhost:9000']"
        )
        .is_err()
    );
}

#[test]
fn default_interface_url_matches_the_vite_server_address() {
    let vite = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ui/vite.config.ts"
    ))
    .expect("ui/vite.config.ts");
    let server = vite
        .split_once("server: {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(block, _)| block)
        .expect("server block in ui/vite.config.ts");
    let field = |name: &str| {
        server
            .split(',')
            .filter_map(|part| part.split_once(':'))
            .find(|(key, _)| key.trim() == name)
            .map(|(_, value)| value.trim().trim_matches(['\'', '"']).to_owned())
            .unwrap_or_else(|| panic!("server.{name} missing in ui/vite.config.ts"))
    };
    let base = ApiConfig::from_toml_str("").unwrap().ui_base_url;
    assert_eq!(
        base.trim_end_matches('/'),
        format!("http://{}:{}", field("host"), field("port")),
        "ui_base_url and the Vite server address diverge"
    );
}

#[test]
fn pending_auth_default_allows_thirty_two_connections() {
    assert_eq!(ApiConfig::from_toml_str("").unwrap().max_pending_auth, 32);
}
