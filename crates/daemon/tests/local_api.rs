use ads_contracts::CommandSessionOpenV1Params;
use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher, Launcher, LocalApi};
use ads_daemon::auth::Token;
use ads_daemon::events::{EventLog, new_id};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server, wire::OpenParams};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

type Socket = WebSocketStream<TcpStream>;
struct TestLauncher;
impl Launcher for TestLauncher {
    fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
        serde_json::from_value(
            json!({"session_id": request.session_id, "cols": request.cols,
            "rows": request.rows, "program": "fake-terminal", "args": [], "cwd": null, "env": {}}),
        )
        .map_err(|_| ApiError::Invalid)
    }
}
struct Fixture {
    port: u16,
    token: String,
    backend: Arc<FakeBackend>,
    host: Arc<PtyHost>,
    api: tokio::task::JoinHandle<Result<(), ApiError>>,
    peer: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for terminal in self.backend.terminals() {
            terminal.exit(0);
        }
        self.api.abort();
        self.peer.abort();
    }
}
async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .expect("test timeout")
}
async fn fixture(overrides: &str, launcher: Arc<dyn Launcher>) -> Fixture {
    fixture_with_host(overrides, launcher, HostConfig::defaults()).await
}
async fn fixture_with_host(
    overrides: &str,
    launcher: Arc<dyn Launcher>,
    host_config: HostConfig,
) -> Fixture {
    let backend = Arc::new(FakeBackend::with_echo());
    let host = Arc::new(PtyHost::new(backend.clone(), host_config).unwrap());
    let (near, far) = tokio::io::duplex(262144);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = bounded(PtyClient::over(
        near,
        ClientConfig::recommended(host.config()),
    ))
    .await
    .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str(overrides).unwrap(),
        client,
        launcher,
    )
    .await
    .unwrap();
    let port = api.port().unwrap();
    let url = api.launch_url().unwrap();
    let (http, fragment) = url.split_once('#').unwrap();
    assert_eq!(http, "http://127.0.0.1:5173/");
    assert!(!http.contains("token"));
    let (core, token) = fragment.split_once("&token=").unwrap();
    assert_eq!(core, format!("core=ws://127.0.0.1:{port}"));
    assert_eq!(token.len(), 43);
    let token = token.to_owned();
    Fixture {
        port,
        token,
        backend,
        host,
        api: tokio::spawn(api.run()),
        peer,
    }
}
async fn connect(
    f: &Fixture,
    origin: Option<&str>,
    host: Option<&str>,
) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://127.0.0.1:{}/", f.port)
        .into_client_request()
        .unwrap();
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("Origin", origin.parse().unwrap());
    }
    if let Some(host) = host {
        request.headers_mut().insert("Host", host.parse().unwrap());
    }
    let stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    bounded(tokio_tungstenite::client_async(request, stream))
        .await
        .map(|(ws, _)| ws)
}
fn command(method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({"idempotency_key": new_id().unwrap(), "origin": "local"});
    json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "method": method, "params": params})
}
async fn transmit(ws: &mut Socket, value: &Value) {
    bounded(ws.send(Message::Text(value.to_string().into())))
        .await
        .unwrap();
}
async fn receive(ws: &mut Socket) -> Value {
    let message = bounded(ws.next()).await.unwrap().unwrap();
    let Message::Text(text) = message else {
        panic!("expected text")
    };
    let value: Value = serde_json::from_str(&text).unwrap();
    assert!(
        ads_contracts::reencode("urn:ads:contract:local-api.server-message:1", value.clone())
            .unwrap()
            .is_ok()
    );
    if let Some(schema) = value["type"].as_str() {
        assert!(
            ads_contracts::reencode(schema, value.clone())
                .unwrap()
                .is_ok()
        );
    }
    value
}
async fn call(ws: &mut Socket, method: &str, params: Value) -> Value {
    let value = command(method, params);
    transmit(ws, &value).await;
    let response = receive(ws).await;
    assert_eq!(response["id"], value["id"]);
    response
}
async fn authenticated(f: &Fixture) -> Socket {
    let mut ws = connect(f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    assert_eq!(
        call(&mut ws, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
    ws
}
fn open_params(id: &str) -> Value {
    json!({"session_id": id, "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24})
}
async fn closed(ws: &mut Socket) {
    assert!(matches!(
        bounded(ws.next()).await,
        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
    ));
}

#[tokio::test]
async fn upgrade_rejects_wrong_or_missing_origin_and_host() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    for (origin, host) in [
        (Some("https://untrusted.invalid"), None),
        (None, None),
        (Some("http://127.0.0.1:5173"), Some("untrusted.invalid")),
        (Some("null"), None),
        (Some("http://127.0.0.1:5173.attacker.invalid"), None),
    ] {
        let error = connect(&f, origin, host)
            .await
            .expect_err("upgrade accepted");
        match error {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), 403)
            }
            _ => panic!("expected 403"),
        }
    }
    for origin in ["http://127.0.0.1:5173"] {
        for host in [
            format!("localhost:{}", f.port),
            format!("127.0.0.1:{}", f.port),
        ] {
            let mut ws = connect(&f, Some(origin), Some(&host)).await.unwrap();
            ws.close(None).await.unwrap();
        }
    }
}

#[tokio::test]
async fn authentication_rejects_missing_wrong_late_token_and_command_before_hello() {
    let f = fixture("auth_timeout_ms = 50", Arc::new(TestLauncher)).await;
    for value in [
        command("auth.hello", json!({})),
        command("auth.hello", json!({"token": "A".repeat(43)})),
        command("session.open", open_params("unauthenticated")),
        command("auth.hello", json!({"token": f.token, "extra": true})),
    ] {
        let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .unwrap();
        transmit(&mut ws, &value).await;
        closed(&mut ws).await;
    }
    let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    closed(&mut ws).await;
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn remote_and_unknown_methods_are_refused_and_default_launcher_is_disabled() {
    let f = fixture("", Arc::new(DisabledLauncher)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("disabled")).await["error"]["code"],
        -32011
    );
    for method in ["fs.delete", "fs.write", "unknown"] {
        assert_eq!(
            call(&mut ws, method, json!({})).await["error"]["code"],
            -32601
        );
    }
    let mut remote = command("session.open", open_params("remote"));
    remote["params"]["_meta"]["origin"] = json!("remote");
    remote["params"]["_meta"]["remote"] = json!({"device_id": "other", "signature": STANDARD.encode([0;64]), "nonce": STANDARD.encode([0;16]), "counter": 1, "expires_at": "2030-01-01T00:00:00Z"});
    transmit(&mut ws, &remote).await;
    assert_eq!(receive(&mut ws).await["error"]["code"], -32010);
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn idempotency_prevents_duplicate_writes_across_connections_and_conflicting_payloads() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
    let mut write = command(
        "session.write",
        json!({"session_id": "session", "data": STANDARD.encode(b"once")}),
    );
    transmit(&mut ws, &write).await;
    assert_eq!(receive(&mut ws).await["result"]["replayed"], false);
    let mut second = authenticated(&f).await;
    write["id"] = json!(new_id().unwrap());
    transmit(&mut second, &write).await;
    assert_eq!(receive(&mut second).await["result"]["replayed"], true);
    assert_eq!(f.backend.terminals()[0].input(), b"once");
    write["params"]["data"] = json!(STANDARD.encode(b"different"));
    transmit(&mut second, &write).await;
    assert_eq!(receive(&mut second).await["error"]["code"], -32012);
    assert_eq!(f.backend.terminals()[0].input(), b"once");
    call(&mut ws, "session.close", json!({"session_id": "session"})).await;
}

#[tokio::test]
async fn fake_session_emits_valid_ordered_events_and_replays_after_sequence() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut control = authenticated(&f).await;
    let mut events = authenticated(&f).await;
    assert_eq!(
        call(
            &mut events,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    call(&mut control, "session.open", open_params("session")).await;
    let created = receive(&mut events).await;
    assert_eq!(created["seq"], 1);
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
    for (offset, bytes) in [(0, b"hello".as_slice()), (5, &[0, 255, 1][..])] {
        call(
            &mut control,
            "session.write",
            json!({"session_id": "session", "data": STANDARD.encode(bytes)}),
        )
        .await;
        let event = receive(&mut events).await;
        assert_eq!(event["data"]["offset"], offset);
        assert_eq!(event["data"]["length"], bytes.len());
        assert_eq!(event["data"]["payload"], STANDARD.encode(bytes));
        assert_eq!(event["seq"], if offset == 0 { 2 } else { 3 });
    }
    call(
        &mut control,
        "session.resize",
        json!({"session_id": "session", "cols": 100, "rows": 30}),
    )
    .await;
    let resized = receive(&mut events).await;
    assert_eq!(resized["seq"], 4);
    assert_eq!(resized["type"], "urn:ads:event:session.resized:1");
    assert_eq!(f.backend.terminals()[0].size().cols, 100);
    call(
        &mut control,
        "session.close",
        json!({"session_id": "session"}),
    )
    .await;
    let ended = receive(&mut events).await;
    assert_eq!(ended["seq"], 5);
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert!(f.backend.terminals()[0].was_killed());
    let mut replay = authenticated(&f).await;
    call(
        &mut replay,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 3}),
    )
    .await;
    assert_eq!(receive(&mut replay).await, resized);
    assert_eq!(receive(&mut replay).await, ended);
}

#[tokio::test]
async fn natural_exit_follows_all_output() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("natural")).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/natural", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    f.backend.terminals()[0].emit(b"last");
    f.backend.terminals()[0].exit(7);
    assert_eq!(
        receive(&mut ws).await["type"],
        "urn:ads:event:session.output:1"
    );
    let ended = receive(&mut ws).await;
    assert_eq!(ended["seq"], 3);
    assert_eq!(ended["data"]["exit_code"], 7);
    assert_eq!(ended["data"]["reason"], "exit");
}

#[tokio::test]
async fn session_write_and_replay_limits_are_enforced() {
    let f = fixture(
        "max_sessions = 1\nmax_write_bytes = 4\nevents_per_stream = 2",
        Arc::new(TestLauncher),
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("limited")).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("extra")).await["error"]["code"],
        -32003
    );
    assert_eq!(
        call(
            &mut ws,
            "session.write",
            json!({"session_id": "limited", "data": STANDARD.encode(b"large")})
        )
        .await["error"]["code"],
        -32003
    );
    assert!(f.backend.terminals()[0].input().is_empty());
    for cols in [81, 82, 83] {
        call(
            &mut ws,
            "session.resize",
            json!({"session_id": "limited", "cols": cols, "rows": 24}),
        )
        .await;
    }
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/limited", "after_seq": 1})
        )
        .await["error"]["code"],
        -32004
    );
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/limited", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 3);
    assert_eq!(receive(&mut ws).await["seq"], 4);
    assert_eq!(f.host.sessions().len(), 1);
}

#[tokio::test]
async fn decoded_write_limit_rejects_base64_that_fits_schema_but_exceeds_bytes() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("limit")).await;
    assert_eq!(
        call(
            &mut ws,
            "session.write",
            json!({"session_id": "limit", "data": STANDARD.encode(vec![1; 65536])})
        )
        .await["result"]["accepted"],
        true
    );
    let response = call(
        &mut ws,
        "session.write",
        json!({"session_id": "limit", "data": STANDARD.encode(vec![1; 65538])}),
    )
    .await;
    assert_eq!(response["error"]["code"], -32003);
    assert_eq!(f.backend.terminals()[0].input().len(), 65536);
}

#[tokio::test]
async fn idempotency_cache_evicts_oldest_key() {
    let f = fixture("idempotency_keys = 1", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("cache")).await;
    let a = command(
        "session.write",
        json!({"session_id": "cache", "data": "YQ=="}),
    );
    transmit(&mut ws, &a).await;
    receive(&mut ws).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "cache", "data": "Yg=="}),
    )
    .await;
    transmit(&mut ws, &a).await;
    assert_eq!(receive(&mut ws).await["result"]["replayed"], false);
    assert_eq!(f.backend.terminals()[0].input(), b"aba");
}

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
fn credentials_only_appear_in_launch_fragment_and_diagnostics_are_redacted() {
    let token = Token::generate().unwrap();
    let url = token.launch_url("http://127.0.0.1:5173", 12345);
    let (http, fragment) = url.split_once('#').unwrap();
    let secret = fragment.split_once("&token=").unwrap().1;
    assert_eq!(http, "http://127.0.0.1:5173/");
    assert!(!http.contains(secret));
    assert!(token.matches(secret));
    assert!(!token.matches(&"X".repeat(43)));
    assert!(!token.matches(""));
    assert_ne!(
        Token::generate()
            .unwrap()
            .launch_url("http://127.0.0.1:5173", 12345),
        url
    );
    let mut log = EventLog::new(2);
    log.append("secret", "output", json!({"session_id": "secret", "offset": 0, "length": secret.len(), "encoding": "base64", "payload": STANDARD.encode(secret)}), None).unwrap();
    let mut diagnostics = format!("{token:?} {log:?}");
    let event = ads_daemon::pty_client::Event::Output {
        header: ads_daemon::pty_client::OutputHeader {
            session_id: "secret".to_owned().try_into().unwrap(),
            seq: 1.try_into().unwrap(),
            length: (secret.len() as u32).try_into().unwrap(),
        },
        bytes: secret.as_bytes().to_vec(),
    };
    diagnostics.push_str(&format!("{event:?}"));
    for error in [
        ApiError::Invalid,
        ApiError::Method,
        ApiError::Remote,
        ApiError::HarnessNotConfigured,
        ApiError::Capacity,
        ApiError::Conflict,
        ApiError::Replay,
        ApiError::Unavailable,
    ] {
        diagnostics.push_str(&format!("{error:?} {error}"));
    }
    let terminal = "private-terminal-output-sentinel";
    let error = log.append("secret", "output", json!({"session_id": "secret", "offset": 0, "length": 1, "encoding": "base64", "payload": STANDARD.encode(format!("{terminal}{secret}")), "unexpected": secret}), None).unwrap_err();
    diagnostics.push_str(&format!("{error:?} {error}"));
    assert!(!diagnostics.contains(terminal));
    assert!(!diagnostics.contains(&STANDARD.encode(format!("{terminal}{secret}"))));
    assert!(!diagnostics.contains(secret));
    assert!(!diagnostics.contains(&STANDARD.encode(secret)));
    assert!(!diagnostics.contains(&format!("{:?}", secret.as_bytes())));
}

#[test]
fn dependency_logging_cannot_emit_credentials_or_terminal_bytes() {
    assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Off);
}

#[test]
fn event_log_rejects_invalid_schema_and_preserves_replay_bounds() {
    let mut log = EventLog::new(2);
    assert_eq!(log.append("valid", "output", json!({"session_id": "valid", "offset": -1, "length": 0, "encoding": "base64", "payload": "%%%"}), None), Err(ApiError::Invalid));
    for cols in [80, 81, 82] {
        log.append(
            "valid",
            "resized",
            json!({"session_id": "valid", "cols": cols, "rows": 24}),
            None,
        )
        .unwrap();
    }
    assert_eq!(log.after("session/valid", 0).unwrap().len(), 2);
    assert_eq!(log.after("session/valid", 1).unwrap()[0]["seq"], 2);
    assert_eq!(log.after("session/valid", 2).unwrap()[0]["seq"], 3);
    assert!(log.after("session/valid", 3).unwrap().is_empty());
    assert_eq!(log.after("session/valid", 4), Err(ApiError::Replay));
    assert_eq!(log.after("session/missing", 1), Err(ApiError::Replay));
}

#[tokio::test]
async fn connection_and_message_limits_close_excess_clients() {
    let f = fixture(
        "max_connections = 1\nmax_message_bytes = 1024",
        Arc::new(TestLauncher),
    )
    .await;
    let mut ws = authenticated(&f).await;
    let mut excess = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    transmit(
        &mut excess,
        &command("auth.hello", json!({"token": f.token})),
    )
    .await;
    closed(&mut excess).await;
    transmit(
        &mut ws,
        &command("unknown", json!({"padding": "x".repeat(2048)})),
    )
    .await;
    closed(&mut ws).await;
}

#[tokio::test]
async fn host_disconnect_ends_the_api_and_all_authenticated_connections() {
    let mut f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    f.peer.abort();
    assert_eq!(
        bounded(&mut f.api).await.unwrap(),
        Err(ApiError::Unavailable)
    );
    closed(&mut ws).await;
}

#[tokio::test]
async fn missing_duplicate_headers_and_query_credentials_are_rejected_before_upgrade() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let f = fixture("", Arc::new(TestLauncher)).await;
    for (target, headers) in [
        ("/", "Origin: http://127.0.0.1:5173\r\n".to_owned()),
        (
            "/",
            format!(
                "Host: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:5173\r\nOrigin: http://127.0.0.1:5173\r\n",
                f.port
            ),
        ),
        (
            "/",
            format!(
                "Host: 127.0.0.1:{}\r\nHost: localhost:{}\r\nOrigin: http://127.0.0.1:5173\r\n",
                f.port, f.port
            ),
        ),
        (
            "/?token=refused",
            format!(
                "Host: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:5173\r\n",
                f.port
            ),
        ),
    ] {
        let mut stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
        let request = format!(
            "GET {target} HTTP/1.1\r\n{headers}Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        bounded(stream.read_to_end(&mut response)).await.unwrap();
        assert!(response.starts_with(b"HTTP/1.1 403"));
    }
}

#[tokio::test]
async fn unsafe_open_parameters_and_invalid_sizes_never_reach_the_host() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    for extra in ["program", "args", "cwd", "env"] {
        let mut params = open_params("unsafe");
        params[extra] = json!("refused");
        assert_eq!(
            call(&mut ws, "session.open", params).await["error"]["code"],
            -32602
        );
    }
    for cols in [0, 1001] {
        let mut params = open_params("size");
        params["cols"] = json!(cols);
        assert_eq!(
            call(&mut ws, "session.open", params).await["error"]["code"],
            -32602
        );
    }
    assert!(f.backend.terminals().is_empty());
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
    ] {
        assert!(ApiConfig::from_toml_str(&format!("ui_base_url = '{value}'")).is_err());
    }
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

#[tokio::test]
async fn exact_host_and_origin_variants_are_refused() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    for host in [
        format!("LOCALHOST:{}", f.port),
        format!("localhost.:{}", f.port),
        format!("[::1]:{}", f.port),
        "127.0.0.1".into(),
        "localhost".into(),
        format!("localhost:{}", if f.port == 65535 { 1 } else { f.port + 1 }),
    ] {
        assert!(
            connect(&f, Some("http://127.0.0.1:5173"), Some(&host))
                .await
                .is_err(),
            "accepted {host}"
        );
    }
    for origin in [
        "http://localhost:5173",
        "http://127.0.0.1:5174",
        "HTTP://127.0.0.1:5173",
        "http://127.0.0.1.:5173",
        "http://[::1]:5173",
        "http://127.0.0.1",
        "http://127.0.0.1:5173/",
        "https://127.0.0.1:5173",
        "null",
    ] {
        assert!(
            connect(&f, Some(origin), None).await.is_err(),
            "accepted {origin}"
        );
    }
}

#[tokio::test]
async fn authentication_frames_are_single_attempt_and_ping_works_after_authentication() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    for frame in [
        Message::Ping(vec![1].into()),
        Message::Binary(vec![1].into()),
    ] {
        let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .unwrap();
        ws.send(frame).await.unwrap();
        closed(&mut ws).await;
    }
    for token in [
        "".to_owned(),
        "x".repeat(42),
        "x".repeat(44),
        "x".repeat(43),
    ] {
        let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .unwrap();
        transmit(&mut ws, &command("auth.hello", json!({"token": token}))).await;
        closed(&mut ws).await;
    }
    let mut ws = authenticated(&f).await;
    ws.send(Message::Ping(vec![7, 8].into())).await.unwrap();
    assert_eq!(
        bounded(ws.next()).await.unwrap().unwrap(),
        Message::Pong(vec![7, 8].into())
    );
    transmit(&mut ws, &command("auth.hello", json!({"token": f.token}))).await;
    closed(&mut ws).await;
    let mut ws = authenticated(&f).await;
    ws.send(Message::Binary(vec![1].into())).await.unwrap();
    closed(&mut ws).await;
}

fn remote(mut value: Value) -> Value {
    value["params"]["_meta"]["origin"] = json!("remote");
    value["params"]["_meta"]["remote"] = json!({"device_id": "other", "signature": STANDARD.encode([0;64]), "nonce": STANDARD.encode([0;16]), "counter": 1, "expires_at": "2030-01-01T00:00:00Z"});
    value
}
#[tokio::test]
async fn remote_commands_never_change_sessions_or_subscriptions() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("local")).await;
    for (method, params) in [
        (
            "session.write",
            json!({"session_id": "local", "data": STANDARD.encode(b"secret-terminal")}),
        ),
        (
            "session.resize",
            json!({"session_id": "local", "cols": 90, "rows": 30}),
        ),
        ("session.close", json!({"session_id": "local"})),
        (
            "events.subscribe",
            json!({"stream": "session/local", "after_seq": 0}),
        ),
    ] {
        transmit(&mut ws, &remote(command(method, params))).await;
        assert_eq!(receive(&mut ws).await["error"]["code"], -32010);
    }
    assert!(f.backend.terminals()[0].input().is_empty());
    assert_eq!(f.backend.terminals()[0].size().cols, 80);
    assert!(!f.backend.terminals()[0].was_killed());
    let mut unauth = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    transmit(
        &mut unauth,
        &remote(command("auth.hello", json!({"token": f.token}))),
    )
    .await;
    closed(&mut unauth).await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "local"})).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn subscriptions_have_a_per_connection_limit() {
    let f = fixture("max_sessions = 1", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    for stream in ["session/a", "session/a"] {
        assert_eq!(
            call(
                &mut ws,
                "events.subscribe",
                json!({"stream": stream, "after_seq": 0})
            )
            .await["result"]["accepted"],
            true
        );
    }
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0})
        )
        .await["error"]["code"],
        -32003
    );
    let mut other = authenticated(&f).await;
    assert_eq!(
        call(
            &mut other,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn unauthenticated_sockets_have_separate_bounded_slots() {
    let f = fixture(
        "max_connections = 1\nmax_pending_auth = 2\nauth_timeout_ms = 200",
        Arc::new(TestLauncher),
    )
    .await;
    let mut ws = authenticated(&f).await;
    let a = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    let b = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .is_err()
    );
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    tokio::time::sleep(Duration::from_millis(220)).await;
    drop((a, b));
    let mut available = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    available.close(None).await.unwrap();
}

#[tokio::test]
async fn handshake_and_hello_share_one_deadline() {
    use tokio::io::AsyncWriteExt;
    let f = fixture("auth_timeout_ms = 400", Arc::new(TestLauncher)).await;
    let mut stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    stream.write_all(b"G").await.unwrap();
    tokio::time::sleep(Duration::from_millis(280)).await;
    // Complete the pending HTTP request with the client codec without writing its G twice.
    let request = format!(
        "ET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:5173\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        f.port
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    use tokio::io::AsyncReadExt;
    let start = tokio::time::Instant::now();
    let mut bytes = Vec::new();
    bounded(stream.read_to_end(&mut bytes)).await.unwrap();
    assert!(bytes.starts_with(b"HTTP/1.1 101"));
    assert!(start.elapsed() < Duration::from_millis(300));
}

// A peer implements the public codec and injects refusals independent of terminal timing.
async fn refusing_fixture(
    overrides: &str,
    target: &'static str,
    code: ads_ptyhost::wire::ErrorCode,
    burst: usize,
) -> (Fixture, Arc<std::sync::Mutex<Vec<String>>>) {
    use ads_ptyhost::wire::{self, Call, ResultBody};
    use tokio::io::AsyncWriteExt;
    let backend = Arc::new(FakeBackend::new());
    let host = Arc::new(PtyHost::new(backend.clone(), HostConfig::defaults()).unwrap());
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = calls.clone();
    let error_text = Arc::new(std::sync::Mutex::new(String::new()));
    let peer_error = error_text.clone();
    let (near, mut far) = tokio::io::duplex(4096);
    let peer_host = host.clone();
    let peer = tokio::spawn(async move {
        let mut refused = false;
        let mut subscriptions = 0;
        while let Ok(Some(wire::Frame::Json(payload))) = wire::read_frame(&mut far, 131072).await {
            let request = wire::Request::parse(&payload).unwrap();
            let method = match &request.call {
                Call::Hello(_) => "hello",
                Call::Open(_) => "open",
                Call::Write(_) => "write",
                Call::Resize(_) => "resize",
                Call::Close(_) => "close",
                Call::Subscribe(_) => "subscribe",
                Call::List(_) => "list",
            };
            observed.lock().unwrap().push(method.to_owned());
            if method == "subscribe" {
                subscriptions += 1;
            }
            let outcome = if (method == target
                || (target == "recover" && method == "subscribe" && subscriptions > 1))
                && !refused
            {
                refused = true;
                Err(wire::ErrorBody::new(
                    code,
                    &peer_error.lock().unwrap(),
                    Some(wire::ErrorData {
                        oldest_seq: Some(1.try_into().unwrap()),
                        next_seq: Some(1.try_into().unwrap()),
                        limit: None,
                    }),
                ))
            } else {
                Ok(match &request.call {
                    Call::Hello(_) => ResultBody::Hello(wire::HelloResult {
                        contract: Default::default(),
                    }),
                    Call::Subscribe(_) => ResultBody::ReplayBounds(wire::ReplayBounds {
                        oldest_seq: 1.try_into().unwrap(),
                        next_seq: 1.try_into().unwrap(),
                    }),
                    Call::Close(_) => ResultBody::Closed(wire::Closed { exit_code: None }),
                    _ => ResultBody::Empty(wire::Empty {}),
                })
            };
            if outcome.is_ok() && (target.starts_with("gap-") || target == "recover") {
                match &request.call {
                    Call::Open(params) => {
                        peer_host
                            .open(
                                &params.session_id,
                                &ads_ptyhost::SpawnSpec {
                                    program: params.program.clone().into(),
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                    }
                    Call::Close(params) => {
                        peer_host.close(&params.session_id).await.unwrap();
                    }
                    _ => (),
                }
            }
            if let Call::Close(params) = &request.call
                && target == "exit-before-close-response"
            {
                far.write_all(
                    &wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
                        session_id: params.session_id.clone(),
                        last_seq: 0,
                        exit_code: Some(0),
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if let Call::Write(params) = &request.call {
                if target == "stall" {
                    std::future::pending::<()>().await;
                }
                if target == "recover" {
                    far.write_all(
                        &wire::encode_json(&wire::Notification::SubscriptionClosed(
                            wire::SubscriptionClosed {
                                session_id: params.session_id.clone(),
                                last_seq: 0,
                                reason: wire::CloseReason::SlowSubscriber,
                            },
                        ))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                }
                match target {
                    "gap-output" => far
                        .write_all(
                            &wire::encode_output(&params.session_id, 2, b"terminal-sentinel")
                                .unwrap(),
                        )
                        .await
                        .unwrap(),
                    "gap-exit" => far
                        .write_all(
                            &wire::encode_json(&wire::Notification::SessionExited(
                                wire::SessionExited {
                                    session_id: params.session_id.clone(),
                                    last_seq: 1,
                                    exit_code: Some(0),
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap(),
                    "gap-close" => far
                        .write_all(
                            &wire::encode_json(&wire::Notification::SubscriptionClosed(
                                wire::SubscriptionClosed {
                                    session_id: params.session_id.clone(),
                                    last_seq: 1,
                                    reason: wire::CloseReason::SessionClosed,
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap(),
                    _ => (),
                }
                for seq in 1..=burst {
                    far.write_all(
                        &wire::encode_output(&params.session_id, seq as u64, b"terminal-sentinel")
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                }
                if burst > 0 {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            }
            if far
                .write_all(
                    &wire::encode_json(&wire::Response {
                        id: Some(request.id),
                        outcome,
                    })
                    .unwrap(),
                )
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let client = PtyClient::over(
        near,
        ClientConfig {
            max_frame_bytes: 131072,
            queue_capacity: 2,
        },
    )
    .await
    .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str(overrides).unwrap(),
        client,
        Arc::new(TestLauncher),
    )
    .await
    .unwrap();
    let token = api
        .launch_url()
        .unwrap()
        .split_once("&token=")
        .unwrap()
        .1
        .to_owned();
    *error_text.lock().unwrap() = format!("{token} credential-and-terminal-sentinel");
    (
        Fixture {
            port: api.port().unwrap(),
            token,
            backend,
            host,
            api: tokio::spawn(api.run()),
            peer,
        },
        calls,
    )
}

#[tokio::test]
async fn every_host_refusal_returns_its_code_and_service_survives() {
    use ads_ptyhost::wire::ErrorCode::*;
    for code in [
        ParseError,
        InvalidRequest,
        MethodNotFound,
        InvalidParams,
        InternalError,
        UnknownSession,
        SessionExists,
        TooManySessions,
        ReplayEvicted,
        ReplayAhead,
        SessionEnded,
        SpawnFailed,
        TerminalIo,
        UnsupportedContract,
    ] {
        for target in ["open", "write", "resize", "close", "subscribe"] {
            let (f, _) = refusing_fixture("", target, code, 0).await;
            let mut ws = authenticated(&f).await;
            let result = call(&mut ws, "session.open", open_params("session")).await;
            let result = match target {
                "write" => call(
                    &mut ws,
                    "session.write",
                    json!({"session_id": "session", "data": STANDARD.encode(f.token.as_bytes())}),
                )
                .await,
                "resize" => {
                    call(
                        &mut ws,
                        "session.resize",
                        json!({"session_id": "session", "cols": 90, "rows": 30}),
                    )
                    .await
                }
                "close" => call(&mut ws, "session.close", json!({"session_id": "session"})).await,
                _ => result,
            };
            assert_eq!(result["error"]["code"], code.number(), "{target}: {code:?}");
            if target == "close" && code != UnknownSession {
                assert_eq!(
                    call(&mut ws, "session.close", json!({"session_id": "session"})).await["result"]
                        ["accepted"],
                    true
                );
            }
            let diagnostics = result.to_string();
            assert!(!diagnostics.contains(&f.token));
            assert!(!diagnostics.contains("credential-and-terminal-sentinel"));
            assert_eq!(
                call(
                    &mut ws,
                    "events.subscribe",
                    json!({"stream": "session/alive", "after_seq": 0})
                )
                .await["result"]["accepted"],
                true
            );
            assert!(!f.api.is_finished());
        }
    }
}

#[tokio::test]
async fn failed_open_subscription_closes_host_and_allows_retry() {
    use ads_ptyhost::wire::ErrorCode;
    let (f, calls) = refusing_fixture("", "subscribe", ErrorCode::SessionEnded, 0).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32006
    );
    assert_eq!(
        &*calls.lock().unwrap(),
        &["hello", "open", "subscribe", "close"]
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn output_is_consumed_during_host_wait_and_overflow_notifies_slow_subscriber() {
    let (f, _) = refusing_fixture(
        "events_per_stream = 2\nhost_timeout_ms = 500",
        "never",
        ads_ptyhost::wire::ErrorCode::InternalError,
        40,
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    let subscription = command(
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    );
    transmit(&mut ws, &subscription).await;
    assert_eq!(receive(&mut ws).await["result"]["accepted"], true);
    assert_eq!(receive(&mut ws).await["seq"], 1);
    let result = call(
        &mut ws,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    assert_eq!(result["result"]["accepted"], true);
    let error = receive(&mut ws).await;
    assert_eq!(error["id"], subscription["id"]);
    assert_eq!(error["error"]["code"], -32004);
    closed(&mut ws).await;
    let mut other = authenticated(&f).await;
    assert_eq!(
        call(&mut other, "unknown", json!({})).await["error"]["code"],
        -32601
    );
}

#[tokio::test]
async fn fake_backend_existing_session_and_full_host_do_not_stop_service() {
    let f = fixture_with_host(
        "",
        Arc::new(TestLauncher),
        HostConfig::from_toml_str("max_sessions = 1").unwrap(),
    )
    .await;
    let id = ads_ptyhost::wire::SessionId::new("existing").unwrap();
    f.host
        .open(
            &id,
            &ads_ptyhost::SpawnSpec {
                program: "fake-terminal".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("existing")).await["error"]["code"],
        -32002
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("extra")).await["error"]["code"],
        -32003
    );
    f.host.close(&id).await.unwrap();
    assert_eq!(
        call(&mut ws, "session.open", open_params("new")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn unknown_session_on_close_releases_the_slot() {
    use ads_ptyhost::wire::ErrorCode::UnknownSession;
    let (f, _) = refusing_fixture("max_sessions = 1", "close", UnknownSession, 0).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["error"]["code"],
        UnknownSession.number()
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn ended_sessions_can_be_closed_and_return_their_slots() {
    let f = fixture("max_sessions = 2", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    for (index, id) in ["first", "second"].into_iter().enumerate() {
        call(&mut ws, "session.open", open_params(id)).await;
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": format!("session/{id}"), "after_seq": 0}),
        )
        .await;
        assert_eq!(receive(&mut ws).await["seq"], 1);
        f.backend.terminals()[index].exit(0);
        assert_eq!(
            receive(&mut ws).await["type"],
            "urn:ads:event:session.ended:1"
        );
    }
    assert_eq!(
        call(&mut ws, "session.open", open_params("third")).await["error"]["code"],
        -32003
    );
    assert_eq!(f.host.sessions().len(), 2);
    for id in ["first", "second"] {
        assert_eq!(
            call(&mut ws, "session.close", json!({"session_id": id})).await["result"]["accepted"],
            true
        );
    }
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "first"})).await["error"]["code"],
        -32602
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("third")).await["result"]["accepted"],
        true
    );
}

async fn assert_invalidated_session_closed(f: &Fixture, ws: &mut Socket) {
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert_eq!(
        call(ws, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn failed_host_resubscribe_invalidates_only_the_affected_stream() {
    let (f, _) = refusing_fixture(
        "max_sessions = 1",
        "recover",
        ads_ptyhost::wire::ErrorCode::ReplayEvicted,
        0,
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    // A following request also acts as a barrier after the recovery refusal.
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["error"]["code"],
        -32004
    );
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/other", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_invalidated_session_closed(&f, &mut ws).await;
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn host_timeout_ends_the_service_after_an_uncertain_command() {
    let (mut f, _) = refusing_fixture(
        "host_timeout_ms = 50",
        "stall",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    transmit(
        &mut ws,
        &command(
            "session.write",
            json!({"session_id": "session", "data": "YQ=="}),
        ),
    )
    .await;
    assert_eq!(
        bounded(&mut f.api).await.unwrap(),
        Err(ApiError::Unavailable)
    );
    closed(&mut ws).await;
}

#[tokio::test]
async fn host_event_gaps_refuse_replay_without_stopping_the_service() {
    for target in ["gap-output", "gap-exit", "gap-close"] {
        let (f, _) = refusing_fixture(
            "max_sessions = 1",
            target,
            ads_ptyhost::wire::ErrorCode::InternalError,
            0,
        )
        .await;
        let mut ws = authenticated(&f).await;
        call(&mut ws, "session.open", open_params("session")).await;
        assert_eq!(
            call(
                &mut ws,
                "session.write",
                json!({"session_id": "session", "data": "YQ=="})
            )
            .await["result"]["accepted"],
            true
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            call(
                &mut ws,
                "events.subscribe",
                json!({"stream": "session/session", "after_seq": 0})
            )
            .await["error"]["code"],
            -32004
        );
        assert_eq!(
            call(&mut ws, "unknown", json!({})).await["error"]["code"],
            -32601
        );
        assert_invalidated_session_closed(&f, &mut ws).await;
        assert!(!f.api.is_finished());
    }
}

#[tokio::test]
async fn exit_before_close_reply_is_reported_as_requested_termination() {
    let (f, _) = refusing_fixture(
        "",
        "exit-before-close-response",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut events = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    call(
        &mut events,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut events).await["seq"], 1);
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "session"})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(receive(&mut events).await["data"]["reason"], "killed");
}
