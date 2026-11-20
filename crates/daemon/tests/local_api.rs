use ads_contracts::CommandSessionOpenV1Params;
use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher, Launcher, LocalApi, TestHooks};
use ads_daemon::auth::Token;
use ads_daemon::events::{EventLog, Poll, new_id};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_daemon::tool_launcher::ToolLauncher;
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
    hooks: Arc<TestHooks>,
    /// Lets a scripted host send the reply it is holding.
    release: Arc<tokio::sync::Notify>,
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
        hooks: api.test_hooks(),
        release: Arc::default(),
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
    receive_reply(ws, false).await
}
/// `listed`: the frame answers a harness.list request, the only one allowed outside version 1.
async fn receive_reply(ws: &mut Socket, listed: bool) -> Value {
    let message = bounded(ws.next()).await.unwrap().unwrap();
    let Message::Text(text) = message else {
        panic!("expected text")
    };
    let value: Value = serde_json::from_str(&text).unwrap();
    assert!(
        ads_contracts::reencode("urn:ads:contract:local-api.server-message:2", value.clone())
            .unwrap()
            .is_ok()
    );
    if listed && value.get("result").is_some() {
        assert!(
            ads_contracts::reencode("urn:ads:command:harness.list.response:1", value.clone())
                .unwrap()
                .is_ok(),
            "{value}"
        );
    } else {
        assert!(
            ads_contracts::reencode("urn:ads:contract:local-api.server-message:1", value.clone())
                .unwrap()
                .is_ok(),
            "{value}"
        );
    }
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
    let response = receive_reply(ws, method == "harness.list").await;
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
        command("harness.list", json!({})),
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
    let mut log = EventLog::new(2, 4);
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
    let mut log = EventLog::new(2, 4);
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
        ("harness.list", json!({})),
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
    let mut a = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let b = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    // A full pending queue makes room by dropping its oldest entry, never the newcomer.
    let mut newest = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    assert_eof(&mut a).await;
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    newest.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(220)).await;
    drop(b);
    let mut available = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    available.close(None).await.unwrap();
}

async fn assert_eof(stream: &mut TcpStream) {
    use tokio::io::AsyncReadExt;
    let mut byte = [0u8; 1];
    assert!(matches!(
        bounded(stream.read(&mut byte)).await,
        Ok(0) | Err(_)
    ));
}

#[tokio::test]
async fn thirty_third_pending_connection_drops_the_oldest() {
    use tokio::io::AsyncReadExt;
    // A long deadline keeps the authentication timeout from closing the oldest socket instead.
    let f = fixture("auth_timeout_ms = 60000", Arc::new(TestLauncher)).await;
    let mut pending = Vec::new();
    for _ in 0..32 {
        pending.push(TcpStream::connect(("127.0.0.1", f.port)).await.unwrap());
        // Spacing keeps the accept order equal to the connect order.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eof(&mut pending[0]).await;
    let mut byte = [0u8; 1];
    for stream in &mut pending[1..] {
        assert!(
            tokio::time::timeout(Duration::from_millis(5), stream.read(&mut byte))
                .await
                .is_err()
        );
    }
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
    scripted_fixture(overrides, target, code, burst, &[(target, 1)]).await
}

// `refusals` gives how many calls of each method the peer refuses ("recover" counts resubscriptions).
async fn scripted_fixture(
    overrides: &str,
    target: &'static str,
    code: ads_ptyhost::wire::ErrorCode,
    burst: usize,
    refusals: &[(&'static str, usize)],
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
    let release: Arc<tokio::sync::Notify> = Arc::default();
    let peer_release = release.clone();
    let mut budget: std::collections::HashMap<&'static str, usize> =
        refusals.iter().copied().collect();
    let peer = tokio::spawn(async move {
        let mut released = false;
        let mut dropped = false;
        let mut held = false;
        let mut other_exited = false;
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
            let key = if target == "recover" && method == "subscribe" && subscriptions > 1 {
                "recover"
            } else {
                method
            };
            let refuse = budget.get_mut(key).is_some_and(|left| {
                let refuse = *left > 0;
                *left = left.saturating_sub(1);
                refuse
            });
            if method == "subscribe" && target == "host-hold-subscribe" && !held {
                // The host opened the terminal but never answers the subscription.
                held = true;
                continue;
            }
            if method == "subscribe" && target == "host-lost-on-subscribe" {
                // The host goes away after opening the terminal, before answering the subscription.
                break;
            }
            if method == "subscribe" && target == "flood-on-subscribe" {
                // The host stops reading and keeps asking for resubscriptions: the client's
                // outgoing queue fills and the client stops.
                let notice = wire::encode_json(&wire::Notification::SubscriptionClosed(
                    wire::SubscriptionClosed {
                        session_id: wire::SessionId::new("x").unwrap(),
                        last_seq: 0,
                        reason: wire::CloseReason::SlowSubscriber,
                    },
                ))
                .unwrap();
                while far.write_all(&notice).await.is_ok() {}
                break;
            }
            if method == "subscribe" && target == "busy-on-open" {
                // Resubscriptions stay unanswered and take every pending call of the client.
                continue;
            }
            if method == "close" && target.starts_with("exit-other-") && !other_exited {
                // Another session ends while the daemon waits for this close.
                other_exited = true;
                far.write_all(
                    &wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
                        session_id: wire::SessionId::new("b").unwrap(),
                        last_seq: 0,
                        exit_code: Some(0),
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
            }
            if method == "close" && target.ends_with("drop-close") && !dropped {
                // The first close never gets a reply, as if the host stalled on it.
                dropped = true;
                continue;
            }
            let outcome = if refuse {
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
            if outcome.is_ok()
                && (target.starts_with("gap-")
                    || target.starts_with("host-")
                    || target == "recover")
            {
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
                && matches!(
                    target,
                    "exit-before-close-response"
                        | "gap-before-close-response"
                        | "exit-then-hold-close"
                )
            {
                far.write_all(
                    &wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
                        session_id: params.session_id.clone(),
                        last_seq: if target == "gap-before-close-response" {
                            7
                        } else {
                            0
                        },
                        exit_code: Some(0),
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if let Call::Close(params) = &request.call
                && matches!(
                    target,
                    "closed-before-close-response" | "closed-then-hold-close"
                )
            {
                far.write_all(
                    &wire::encode_json(&wire::Notification::SubscriptionClosed(
                        wire::SubscriptionClosed {
                            session_id: params.session_id.clone(),
                            last_seq: 0,
                            reason: wire::CloseReason::SessionClosed,
                        },
                    ))
                    .unwrap(),
                )
                .await
                .unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if matches!(request.call, Call::Close(_))
                && target.ends_with("-then-hold-close")
                && !released
            {
                // The first close reply waits until the test saw the terminal event processed.
                released = true;
                peer_release.notified().await;
            }
            if matches!(request.call, Call::Open(_)) && target == "busy-on-open" {
                for name in ["x", "y"] {
                    far.write_all(
                        &wire::encode_json(&wire::Notification::SubscriptionClosed(
                            wire::SubscriptionClosed {
                                session_id: wire::SessionId::new(name).unwrap(),
                                last_seq: 0,
                                reason: wire::CloseReason::SlowSubscriber,
                            },
                        ))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                }
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
                    "exit-other-then-hold-close" | "exit-other-then-drop-close"
                        if params.session_id.as_str() == "a" =>
                    {
                        far.write_all(
                            &wire::encode_json(&wire::Notification::SessionExited(
                                wire::SessionExited {
                                    session_id: params.session_id.clone(),
                                    last_seq: 0,
                                    exit_code: Some(0),
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap()
                    }
                    "gap-output" | "gap-output-drop-close" => far
                        .write_all(
                            &wire::encode_output(&params.session_id, 2, b"terminal-sentinel")
                                .unwrap(),
                        )
                        .await
                        .unwrap(),
                    "exit-on-write" => far
                        .write_all(
                            &wire::encode_json(&wire::Notification::SessionExited(
                                wire::SessionExited {
                                    session_id: params.session_id.clone(),
                                    last_seq: 0,
                                    exit_code: Some(0),
                                },
                            ))
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
            hooks: api.test_hooks(),
            release,
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
            // The terminal is already open when the subscription is refused: setup failed, close scheduled.
            let expected = if target == "subscribe" {
                -32014
            } else {
                code.number()
            };
            assert_eq!(result["error"]["code"], expected, "{target}: {code:?}");
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
        -32014
    );
    bounded(async {
        while calls.lock().unwrap().len() < 4 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
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
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["error"]["code"],
        UnknownSession.number()
    );
    // Without the host the stream would stay open forever in the interface.
    let ended = receive(&mut ws).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["seq"], 2);
    assert_eq!(ended["data"]["reason"], "error");
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

/// Waits a bounded time for the host to drop `count` sessions; returns either way so the caller asserts.
async fn host_sessions_settle(f: &Fixture, count: usize) {
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while f.host.sessions().len() != count {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;
}

fn close_calls(calls: &std::sync::Mutex<Vec<String>>) -> usize {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == "close")
        .count()
}
async fn wait_closes(calls: &std::sync::Mutex<Vec<String>>, count: usize) {
    bounded(async {
        while close_calls(calls) < count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn ended_session_returns_its_slot_before_any_client_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1",
        "exit-on-write",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("first")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "first", "data": "YQ=="}),
    )
    .await;
    // The host sees the close before answering it; the daemon reads the answer before the next open.
    wait_closes(&calls, 1).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    // A client that still closes the ended session is answered as before, without reaching the host.
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "first"})).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), 1);
    // A session the client closed itself still refuses a second close.
    assert_eq!(
        call(
            &mut ws,
            "session.close",
            json!({"session_id": "replacement"})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(
            &mut ws,
            "session.close",
            json!({"session_id": "replacement"})
        )
        .await["error"]["code"],
        -32602
    );
}

#[tokio::test]
async fn released_streams_stay_whole_and_pruning_ends_only_its_subscription() {
    let f = fixture("max_sessions = 2", Arc::new(TestLauncher)).await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let mut subscriptions = Vec::new();
    for (index, id) in ["first", "second"].into_iter().enumerate() {
        call(&mut control, "session.open", open_params(id)).await;
        subscriptions.push(
            call(
                &mut watcher,
                "events.subscribe",
                json!({"stream": format!("session/{id}"), "after_seq": 0}),
            )
            .await["id"]
                .clone(),
        );
        assert_eq!(receive(&mut watcher).await["seq"], 1);
        f.backend.terminals()[index].exit(0);
        let ended = receive(&mut watcher).await;
        assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
        assert_eq!(ended["data"]["reason"], "exit");
    }
    host_sessions_settle(&f, 0).await;
    assert!(f.host.sessions().is_empty());
    call(
        &mut control,
        "events.subscribe",
        json!({"stream": "session/first", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut control).await["seq"], 1);
    assert_eq!(receive(&mut control).await["seq"], 2);
    call(&mut control, "session.open", open_params("third")).await;
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "third"})
        )
        .await["result"]["accepted"],
        true
    );
    // Three released records exceed max_sessions = 2: only the oldest is pruned, as before this change.
    let pruned = receive(&mut watcher).await;
    assert_eq!(pruned["id"], subscriptions[0]);
    assert_eq!(pruned["error"]["code"], -32004);
    assert_eq!(receive(&mut control).await["error"]["code"], -32004);
    // The next frame on the watcher answers its own request: the second subscription did not end.
    assert_eq!(
        call(
            &mut watcher,
            "session.close",
            json!({"session_id": "first"})
        )
        .await["error"]["code"],
        -32602
    );
    // The second stream is still retained and replays whole.
    call(
        &mut control,
        "events.subscribe",
        json!({"stream": "session/second", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut control).await["seq"], 1);
    assert_eq!(receive(&mut control).await["seq"], 2);
}

#[tokio::test]
async fn exhausted_release_of_an_ended_session_keeps_its_stream_and_client_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1
close_retries = 2
close_retry_ms = 20",
        "exit-on-write",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 2)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    let ended = receive(&mut ws).await;
    assert_eq!(ended["seq"], 2);
    assert_eq!(ended["data"]["reason"], "exit");
    let closes = || {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count()
    };
    bounded(async {
        while closes() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    // Both automatic attempts were refused: the stream stays whole and the slot stays taken.
    // The next frame answers this request while the existing subscription stays open.
    assert_eq!(
        call(
            &mut ws,
            "session.resize",
            json!({"session_id": "session", "cols": 100, "rows": 30})
        )
        .await["error"]["code"],
        -32602
    );
    let mut late = authenticated(&f).await;
    assert_eq!(
        call(
            &mut late,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0}),
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(receive(&mut late).await["seq"], 1);
    assert_eq!(receive(&mut late).await, ended);
    assert_eq!(
        call(&mut late, "session.open", open_params("other")).await["error"]["code"],
        -32003
    );
    assert_eq!(closes(), 2);
    assert_eq!(
        call(&mut late, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut late, "session.open", open_params("other")).await["result"]["accepted"],
        true
    );
}

async fn terminal_event_during_a_refused_client_close_still_releases(
    target: &'static str,
    reason: &str,
) {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retry_ms = 20",
        target,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 1)],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    call(
        &mut watcher,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    let close = command("session.close", json!({"session_id": "session"}));
    transmit(&mut control, &close).await;
    // The daemon recorded the terminal event while the client's close was still waiting on the host.
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["data"]["reason"], reason);
    f.release.notify_one();
    let refused = receive(&mut control).await;
    assert_eq!(refused["id"], close["id"]);
    assert_eq!(refused["error"]["code"], -32603);
    wait_closes(&calls, 2).await;
    assert_eq!(
        call(&mut control, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), 2);
    assert_eq!(f.hooks.automatic_closes(), 1);
}

#[tokio::test]
async fn exit_during_a_refused_client_close_still_releases_the_slot() {
    terminal_event_during_a_refused_client_close_still_releases("exit-then-hold-close", "killed")
        .await;
}

#[tokio::test]
async fn session_closed_during_a_refused_client_close_still_releases_the_slot() {
    terminal_event_during_a_refused_client_close_still_releases("closed-then-hold-close", "killed")
        .await;
}

#[tokio::test]
async fn unknown_session_on_the_automatic_release_keeps_the_natural_end() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1",
        "exit-on-write",
        ads_ptyhost::wire::ErrorCode::UnknownSession,
        0,
        &[("close", 1)],
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
    wait_closes(&calls, 1).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    let ended = receive(&mut ws).await;
    assert_eq!(ended["seq"], 2);
    assert_eq!(ended["data"]["reason"], "exit");
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), 1);
}

#[tokio::test]
async fn refused_release_of_an_ended_session_never_reaches_a_reopened_id() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nretired_sessions = 1\nclose_retry_ms = 20",
        "exit-on-write",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    // Only the first automatic attempt passes; the repeated one waits at the barrier.
    f.hooks.automatic_close.hold();
    f.hooks.automatic_close.admit_one();
    call(&mut ws, "session.open", open_params("a")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "a", "data": "YQ=="}),
    )
    .await;
    wait_closes(&calls, 1).await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["result"]["accepted"],
        true
    );
    // The second close of "a" was the client's: the daemon sent only the refused one.
    assert_eq!(close_calls(&calls), 2);
    assert_eq!(f.hooks.automatic_closes(), 1);
    // Returning the slot dropped the held attempt.
    assert_eq!(f.hooks.queued_closes(), 0);
    f.hooks.automatic_close.release();
    // Another released session prunes "a"; its tombstone keeps the id taken.
    call(&mut ws, "session.open", open_params("b")).await;
    call(&mut ws, "session.close", json!({"session_id": "b"})).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("a")).await["error"]["code"],
        -32012
    );
    // One more pushes the tombstone out.
    call(&mut ws, "session.open", open_params("c")).await;
    call(&mut ws, "session.close", json!({"session_id": "c"})).await;
    // A late close of the pruned id, with a fresh key, is refused instead of accepted without effect.
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["error"]["code"],
        -32602
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    let closes = close_calls(&calls);
    assert_eq!(f.hooks.queued_closes(), 0);
    // The new generation is still active.
    let resize = json!({"session_id": "a", "cols": 100, "rows": 30});
    assert_eq!(
        call(&mut ws, "session.resize", resize.clone()).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), closes);
    assert_eq!(f.hooks.automatic_closes(), 1);
    // As documented, a close by the reused id refers to the current session.
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), closes + 1);
    assert_eq!(
        call(&mut ws, "session.resize", resize).await["error"]["code"],
        -32602
    );
}

#[tokio::test]
async fn more_sessions_than_the_limit_can_end_on_their_own() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    // The default limit is 32; ending each one on its own must keep the next open working.
    let total = 40;
    for index in 0..total {
        let id = format!("natural-{index}");
        assert_eq!(
            call(&mut ws, "session.open", open_params(&id)).await["result"]["accepted"],
            true,
            "open {index}"
        );
        f.backend.terminals()[index].exit(0);
        host_sessions_settle(&f, 0).await;
    }
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        call(&mut ws, "session.open", open_params("after")).await["result"]["accepted"],
        true
    );
    // The last ended session keeps its full stream for a late subscriber.
    let mut late = authenticated(&f).await;
    call(
        &mut late,
        "events.subscribe",
        json!({"stream": format!("session/natural-{}", total - 1), "after_seq": 0}),
    )
    .await;
    assert_eq!(
        receive(&mut late).await["type"],
        "urn:ads:event:session.created:1"
    );
    let ended = receive(&mut late).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["data"]["reason"], "exit");
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

#[test]
fn unavailable_service_and_host_timeout_share_the_internal_error_code() {
    for error in [ApiError::Unavailable, ApiError::HostTimeout] {
        assert_eq!(error.code(), -32603);
    }
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

struct FakeProbe;

impl ads_core::tool_launch::Probe for FakeProbe {
    fn is_file(&self, path: &str) -> bool {
        matches!(path, r"C:\tools\claude.exe" | r"C:\tools\codex.exe")
    }

    fn is_dir(&self, path: &str) -> bool {
        path == r"C:\workspace"
    }
}

#[tokio::test]
async fn configured_adapters_supply_the_host_with_filtered_parameters() {
    let drive = if cfg!(windows) { "C:" } else { "" };
    let overrides = format!(
        r#"
[accounts.claude.primary]
profile_dir = '{drive}/profiles/claude'
[accounts.codex.primary]
profile_dir = '{drive}/profiles/codex'
[workdirs]
workspace = 'C:/workspace'
[tools.claude]
program = 'C:/tools/claude.exe'
args = ['claude-arg']
[tools.codex]
program = 'C:/tools/codex.exe'
args = ['codex-arg']
"#
    );
    let parent_env = [
        ("PATH", "C:/tools"),
        ("ANTHROPIC_API_KEY", "sentinel"),
        ("OPENAI_API_KEY", "sentinel"),
        ("GH_TOKEN", "sentinel"),
        ("CLAUDE_CONFIG_DIR", "sentinel"),
        ("CODEX_HOME", "sentinel"),
        ("UNLISTED", "sentinel"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .to_vec();
    let launcher =
        ToolLauncher::from_toml_str(&overrides, parent_env, FakeProbe, FakeProbe).unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let mut ws = authenticated(&f).await;
    for (index, harness) in ["claude", "codex"].into_iter().enumerate() {
        let mut params = open_params(harness);
        params["harness"] = json!(harness);
        let reply = call(&mut ws, "session.open", params).await;
        assert_eq!(reply["result"]["accepted"], true, "{reply}");
        let terminals = f.backend.terminals();
        assert_eq!(terminals.len(), index + 1);
        let spec = terminals[index].spec();
        assert_eq!(spec.program, format!(r"C:\tools\{harness}.exe"));
        assert_eq!(spec.args, [format!("{harness}-arg")]);
        assert_eq!(
            spec.cwd.as_deref(),
            Some(std::path::Path::new(r"C:\workspace"))
        );
        assert_eq!(
            spec.env
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
            [
                ("PATH", "C:/tools"),
                ("TERM", "xterm-256color"),
                ("COLORTERM", "truecolor"),
            ]
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .into()
        );
    }
}

#[tokio::test]
async fn unconfigured_and_unknown_tools_are_refused_before_spawning() {
    let launcher =
        ToolLauncher::from_toml_str("", Vec::<(String, String)>::new(), FakeProbe, FakeProbe)
            .unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let mut ws = authenticated(&f).await;
    for (harness, code) in [("claude", -32011), ("codex", -32011), ("unknown", -32602)] {
        let mut params = open_params(harness);
        params["harness"] = json!(harness);
        assert_eq!(
            call(&mut ws, "session.open", params).await["error"]["code"],
            code
        );
    }
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn refused_automatic_close_is_retried_until_the_host_closes() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retry_ms = 20",
        "gap-output",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 2)],
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
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    let closes = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == "close")
        .count();
    assert_eq!(closes, 3);
    // The slot came back without any session.close from the client.
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn automatic_close_timeout_is_retried_without_stopping_the_service() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nhost_timeout_ms = 200\nclose_retry_ms = 20",
        "gap-output-drop-close",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
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
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count(),
        2
    );
    assert!(!f.api.is_finished());
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn invalidation_racing_a_client_close_sends_one_close_to_the_host() {
    let (f, calls) = refusing_fixture(
        "",
        "gap-before-close-response",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    // The queued automatic close runs on the next turns of the service loop.
    for _ in 0..3 {
        assert_eq!(
            call(&mut ws, "unknown", json!({})).await["error"]["code"],
            -32601
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count(),
        1
    );
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn pruned_stream_ends_only_its_subscription_and_its_id_is_not_reused() {
    // Two sessions per run also allow two subscriptions per connection.
    let f = fixture("max_sessions = 2", Arc::new(TestLauncher)).await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let pruned = command(
        "events.subscribe",
        json!({"stream": "session/first", "after_seq": 0}),
    );
    transmit(&mut watcher, &pruned).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/later", "after_seq": 0}),
        )
        .await["result"]["accepted"],
        true
    );
    // The watcher does not read while the control connection prunes and tries to reopen the id.
    for id in ["first", "second", "third"] {
        call(&mut control, "session.open", open_params(id)).await;
        assert_eq!(
            call(&mut control, "session.close", json!({"session_id": id})).await["result"]["accepted"],
            true
        );
    }
    assert_eq!(
        call(&mut control, "session.open", open_params("first")).await["error"]["code"],
        -32012
    );
    bounded(async {
        loop {
            let message = receive(&mut watcher).await;
            if message["id"] == pruned["id"] {
                assert_eq!(message["error"]["code"], -32004);
                break;
            }
            assert_eq!(message["stream"], "session/first");
        }
    })
    .await;
    let opened = call(&mut control, "session.open", open_params("later")).await;
    assert_eq!(opened["result"]["accepted"], true);
    let created = receive(&mut watcher).await;
    assert_eq!(created["stream"], "session/later");
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
}

#[test]
fn retired_stream_refuses_replay_and_keeps_its_id_taken() {
    let mut log = EventLog::new(4, 1);
    log.append(
        "a",
        "resized",
        json!({"session_id": "a", "cols": 80, "rows": 24}),
        None,
    )
    .unwrap();
    // A subscriber that had read seq 1 and did not wake up before the stream was pruned.
    log.retire("a");
    assert!(log.is_retired("session/a"));
    assert_eq!(log.after("session/a", 1), Err(ApiError::Replay));
    assert_eq!(log.after("session/a", 0), Err(ApiError::Replay));
    log.retire("b");
    // The tombstone bound evicts the oldest id.
    assert!(!log.is_retired("session/a"));
    assert!(log.is_retired("session/b"));
}

#[tokio::test]
async fn exhausted_automatic_close_reaches_the_original_subscriber() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retries = 3\nclose_retry_ms = 20",
        "gap-output",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 3)],
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut watcher = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    let subscription = command(
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    );
    transmit(&mut watcher, &subscription).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    call(
        &mut control,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    // While the retries run the subscription waits; the final word is the exhausted close.
    let notice = receive(&mut watcher).await;
    assert_eq!(notice["id"], subscription["id"]);
    assert_eq!(notice["error"]["code"], -32013);
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count(),
        3
    );
    assert_eq!(
        call(&mut watcher, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["error"]["code"],
        -32013
    );
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "session"})
        )
        .await["result"]["accepted"],
        true
    );
    assert!(f.backend.terminals()[0].was_killed());
    assert_eq!(
        call(&mut control, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn failed_open_cleanup_timeout_keeps_the_record_until_the_host_closes() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nhost_timeout_ms = 200\nclose_retry_ms = 20",
        "host-drop-close",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("subscribe", 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    // The host opened a real terminal; the first close never gets a reply.
    assert_eq!(f.host.sessions().len(), 1);
    assert_eq!(
        call(&mut ws, "session.open", open_params("other")).await["error"]["code"],
        -32003
    );
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count(),
        2
    );
    assert!(!f.api.is_finished());
    // Once the host confirmed, the failed open gives its id back.
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn stale_automatic_close_never_reaches_a_reopened_id() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nretired_sessions = 1\nclose_retry_ms = 400",
        "gap-output",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("a")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "a", "data": "YQ=="}),
    )
    .await;
    // The first automatic close is refused; the next attempt waits 400 ms.
    bounded(async {
        while calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count()
            < 1
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["result"]["accepted"],
        true
    );
    // Two more closed sessions prune "a" and push its tombstone out, so the id can be reused.
    for id in ["b", "c"] {
        call(&mut ws, "session.open", open_params(id)).await;
        call(&mut ws, "session.close", json!({"session_id": id})).await;
    }
    assert_eq!(
        call(&mut ws, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    let closes = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == "close")
        .count();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count(),
        closes
    );
    let reopened = f.backend.terminals().last().unwrap().clone();
    assert!(!reopened.was_killed());
    assert_eq!(f.host.sessions().len(), 1);
}

#[test]
fn subscription_keeps_its_incarnation_after_the_tombstone_expires() {
    let mut log = EventLog::new(4, 1);
    let data = json!({"session_id": "a", "cols": 80, "rows": 24});
    log.append("a", "resized", data.clone(), None).unwrap();
    let advanced = log.subscribe("session/a", 1).unwrap();
    let from_start = log.subscribe("session/a", 0).unwrap();
    // The subscribers are held elsewhere while two prunes push out the first tombstone.
    log.retire("a");
    log.retire("b");
    assert!(!log.is_retired("session/a"));
    log.append("a", "resized", data, None).unwrap();
    for stream in [&advanced, &from_start] {
        assert_eq!(stream.poll(0), Poll::End(ApiError::Replay));
        assert_eq!(stream.poll(1), Poll::End(ApiError::Replay));
    }
    // A subscription made before the open follows the incarnation the open fills, and only that one.
    let early = log.subscribe("session/c", 0).unwrap();
    log.append(
        "c",
        "resized",
        json!({"session_id": "c", "cols": 80, "rows": 24}),
        None,
    )
    .unwrap();
    assert!(matches!(early.poll(0), Poll::Events(events) if events.len() == 1));
}

#[test]
fn first_final_result_wins_for_existing_subscribers_only() {
    let mut log = EventLog::new(4, 4);
    let data = json!({"session_id": "a", "cols": 80, "rows": 24});
    log.append("a", "resized", data, None).unwrap();
    let stream = log.subscribe("session/a", 0).unwrap();
    log.invalidate("a", ads_daemon::events::Closing::Pending);
    assert_eq!(stream.poll(0), Poll::Wait);
    log.invalidate("a", ads_daemon::events::Closing::Failed);
    log.invalidate("a", ads_daemon::events::Closing::Closed);
    // The subscriber keeps the exhausted result; a new subscription sees the current state.
    assert_eq!(stream.poll(0), Poll::End(ApiError::ClosePending));
    assert_eq!(log.subscribe("session/a", 0).unwrap_err(), ApiError::Replay);
}

#[tokio::test]
async fn upgraded_connections_keep_their_pending_slot() {
    let f = fixture(
        "max_pending_auth = 2\nauth_timeout_ms = 60000",
        Arc::new(TestLauncher),
    )
    .await;
    // The legitimate client finished the upgrade and is about to send auth.hello.
    let mut legit = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    let mut raw = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    // The queue is full: the newcomer evicts the raw socket, never the upgraded connection.
    let newest = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    assert_eof(&mut raw).await;
    assert_eq!(
        call(&mut legit, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
    drop(newest);
    // With every slot taken by an upgraded connection, the next one is refused.
    let mut first = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    let _second = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    assert!(
        connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .is_err()
    );
    assert_eq!(
        call(&mut first, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
    let mut third = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    third.close(None).await.unwrap();
}

async fn wait_until(mut ready: impl FnMut() -> bool) {
    bounded(async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn subscribe_without_host_reply_keeps_the_record_and_schedules_the_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nhost_timeout_ms = 200\nclose_retry_ms = 20",
        "host-hold-subscribe",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    assert!(!f.api.is_finished());
    wait_until(|| f.backend.terminals()[0].was_killed()).await;
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count(),
        1
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn waiting_stream_eviction_wakes_an_idle_websocket() {
    let f = fixture("retired_sessions = 1", Arc::new(TestLauncher)).await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let first = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &first).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    bounded(watcher.send(Message::Ping(vec![1].into())))
        .await
        .unwrap();
    assert_eq!(
        bounded(watcher.next()).await.unwrap().unwrap(),
        Message::Pong(vec![1].into())
    );
    assert_eq!(
        call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0}),
        )
        .await["result"]["accepted"],
        true
    );
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], first["id"]);
    assert_eq!(ended["error"]["code"], -32004);
}

#[tokio::test]
async fn waiting_stream_survives_cleanup_and_resubscription_to_the_same_id() {
    let (f, _) = scripted_fixture(
        "retired_sessions = 2",
        "subscribe",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("subscribe", 1)],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let first = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &first).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["error"]["code"],
        -32014
    );
    loop {
        let event = receive(&mut watcher).await;
        if event["id"] == first["id"] {
            assert_eq!(event["error"]["code"], -32004);
            break;
        }
        assert_eq!(event["type"], "urn:ads:event:session.created:1");
    }
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/a", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
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
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    let created = receive(&mut watcher).await;
    assert_eq!(created["stream"], "session/a");
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
}

#[tokio::test]
async fn early_subscriber_held_across_reuse_never_follows_the_new_session() {
    let f = fixture(
        "max_sessions = 2\nretired_sessions = 2",
        Arc::new(TestLauncher),
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    // The watcher subscribes before the open and is then held before it delivers anything.
    let early = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &early).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/later", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    f.hooks.before_delivery.hold();
    // While it is held, "a" opens, closes, is pruned and its tombstone expires before the id is opened again.
    for id in ["a", "b", "c", "d", "e"] {
        call(&mut control, "session.open", open_params(id)).await;
        call(&mut control, "session.close", json!({"session_id": id})).await;
    }
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    f.hooks.before_delivery.release();
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], early["id"]);
    assert_eq!(ended["error"]["code"], -32004);
    call(&mut control, "session.close", json!({"session_id": "a"})).await;
    call(&mut control, "session.open", open_params("later")).await;
    let created = receive(&mut watcher).await;
    assert_eq!(created["stream"], "session/later");
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
}

#[tokio::test]
async fn queued_host_output_stays_with_the_session_closed_before_id_reuse() {
    let f = fixture(
        "max_sessions = 1\nretired_sessions = 1",
        Arc::new(TestLauncher),
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut watcher = authenticated(&f).await;
    let subscribe = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &subscribe).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    f.hooks.before_delivery.hold();
    f.backend.terminals()[0].emit(b"previous");
    for id in ["a", "b", "c"] {
        if id != "a" {
            assert_eq!(
                call(&mut control, "session.open", open_params(id)).await["result"]["accepted"],
                true
            );
        }
        assert_eq!(
            call(&mut control, "session.close", json!({"session_id": id})).await["result"]["accepted"],
            true
        );
    }
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    f.hooks.before_delivery.release();
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], subscribe["id"]);
    assert_eq!(ended["error"]["code"], -32004);
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/a", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        receive(&mut watcher).await["type"],
        "urn:ads:event:session.created:1"
    );
    assert_eq!(
        call(
            &mut control,
            "session.write",
            json!({"session_id": "a", "data": STANDARD.encode(b"current")})
        )
        .await["result"]["accepted"],
        true
    );
    let output = receive(&mut watcher).await;
    assert_eq!(output["seq"], 2);
    assert_eq!(output["data"]["offset"], 0);
    assert_eq!(output["data"]["payload"], STANDARD.encode(b"current"));
}

#[tokio::test]
async fn delayed_subscriber_gets_the_exhausted_close_after_a_manual_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retries = 3\nclose_retry_ms = 20",
        "gap-output",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[("close", 3)],
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut watcher = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    f.hooks.before_delivery.hold();
    let subscription = command(
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    );
    transmit(&mut watcher, &subscription).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    call(
        &mut control,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    wait_until(|| {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "close")
            .count()
            == 3
    })
    .await;
    // Wait for the exhausted state, then close by hand before the subscriber reads anything.
    bounded(async {
        while call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0}),
        )
        .await["error"]["code"]
            != -32013
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "session"})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["error"]["code"],
        -32004
    );
    f.hooks.before_delivery.release();
    let notice = receive(&mut watcher).await;
    assert_eq!(notice["id"], subscription["id"]);
    assert_eq!(notice["error"]["code"], -32013);
}

#[tokio::test]
async fn connection_is_not_evicted_after_its_upgrade_response() {
    let f = fixture(
        "max_pending_auth = 1\nauth_timeout_ms = 60000",
        Arc::new(TestLauncher),
    )
    .await;
    // The client already has its 101; the server side is held right after the upgrade.
    f.hooks.after_upgrade.hold();
    let mut legit = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    let mut raw = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    assert_eof(&mut raw).await;
    f.hooks.after_upgrade.release();
    assert_eq!(
        call(&mut legit, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn sequential_admissions_keep_the_eviction_queue_small() {
    let f = fixture("max_pending_auth = 4", Arc::new(TestLauncher)).await;
    for _ in 0..40 {
        let mut ws = authenticated(&f).await;
        ws.close(None).await.unwrap();
    }
    let (live, peak, waiting) = f.hooks.pending();
    assert!(live <= 1, "{live}");
    assert_eq!(peak, 1);
    assert!(waiting <= 2, "{waiting}");
}

struct ListProbe;

impl ads_core::tool_launch::Probe for ListProbe {
    fn is_file(&self, _: &str) -> bool {
        panic!("listing must not probe the disk")
    }

    fn is_dir(&self, _: &str) -> bool {
        panic!("listing must not probe the disk")
    }
}

async fn raw_call(ws: &mut Socket, value: &Value) -> String {
    transmit(ws, value).await;
    let Message::Text(text) = bounded(ws.next()).await.unwrap().unwrap() else {
        panic!("expected text")
    };
    let text = text.to_string();
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(reply["id"], value["id"]);
    for schema in [
        "urn:ads:contract:local-api.server-message:2",
        "urn:ads:command:harness.list.response:1",
    ] {
        assert!(
            ads_contracts::reencode(schema, reply.clone())
                .unwrap()
                .is_ok()
        );
    }
    text
}

#[tokio::test]
async fn harness_list_returns_only_tool_names_and_folder_aliases() {
    let overrides = r#"
[env]
allow = ["PATH", "LIST_SENTINEL_VAR"]
[workdirs]
default = 'C:/dir-sentinel/inner'
second = 'D:/other-sentinel'
[tools.claude]
program = 'C:/prog-sentinel/claude.exe'
args = ['--arg-sentinel']
"#;
    let parent_env = [
        ("PATH", "C:/path-sentinel"),
        ("LIST_SENTINEL_VAR", "value-sentinel"),
        ("ANTHROPIC_API_KEY", "key-sentinel"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .to_vec();
    let launcher = ToolLauncher::from_toml_str(overrides, parent_env, ListProbe, ListProbe);
    let launcher = launcher.unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let mut ws = authenticated(&f).await;
    let request = command("harness.list", json!({}));
    let text = raw_call(&mut ws, &request).await;
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        reply["result"],
        json!({"harnesses": ["claude"], "workdir_aliases": ["default", "second"]})
    );
    for leak in [
        "sentinel", ":/", ":\\", "\\", "PATH", "LIST_", "program", "args", "env", "cwd",
    ] {
        assert!(!text.contains(leak), "{leak} leaked in {text}");
    }
    // Reading twice with the same key answers the list again, not a replay marker.
    let text = raw_call(&mut ws, &request).await;
    assert!(text.contains("\"workdir_aliases\""), "{text}");
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn harness_list_is_empty_without_configured_tools() {
    for launcher in [
        Arc::new(DisabledLauncher) as Arc<dyn Launcher>,
        Arc::new(
            ToolLauncher::from_toml_str("", Vec::<(String, String)>::new(), ListProbe, ListProbe)
                .unwrap(),
        ),
        Arc::new(TestLauncher),
    ] {
        let f = fixture("", launcher).await;
        let mut ws = authenticated(&f).await;
        let reply = call(&mut ws, "harness.list", json!({})).await;
        assert_eq!(
            reply["result"],
            json!({"harnesses": [], "workdir_aliases": []})
        );
    }
}

#[tokio::test]
async fn harness_list_refuses_parameters() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    let reply = call(&mut ws, "harness.list", json!({"program": "x"})).await;
    assert_eq!(reply["error"]["code"], -32602);
    assert_eq!(
        call(&mut ws, "harness.list", json!({})).await["result"]["harnesses"],
        json!([])
    );
}

#[tokio::test]
async fn harness_list_needs_the_allowed_page_authentication_and_local_origin() {
    let overrides = "[workdirs]\ndefault = 'C:/dir-sentinel'\n[tools.codex]\nprogram = 'C:/prog-sentinel/codex.exe'\n";
    let launcher = ToolLauncher::from_toml_str(
        overrides,
        Vec::<(String, String)>::new(),
        ListProbe,
        ListProbe,
    )
    .unwrap();
    let f = fixture("", Arc::new(launcher)).await;
    let expected = json!({"harnesses": ["codex"], "workdir_aliases": ["default"]});

    match connect(&f, Some("https://untrusted.invalid"), None).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 403)
        }
        _ => panic!("expected 403"),
    }

    let mut early = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    transmit(&mut early, &command("harness.list", json!({}))).await;
    closed(&mut early).await;

    let mut ws = authenticated(&f).await;
    transmit(&mut ws, &remote(command("harness.list", json!({})))).await;
    let refused = receive(&mut ws).await;
    assert_eq!(refused["error"]["code"], -32010);
    assert!(refused.get("result").is_none());
    assert_eq!(
        call(&mut ws, "harness.list", json!({})).await["result"],
        expected
    );
}

#[tokio::test]
async fn a_client_that_never_lists_tools_only_receives_first_version_frames() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    // `authenticated` and `call` check every reply against version 1 as well.
    let mut ws = authenticated(&f).await;
    let mut events = authenticated(&f).await;
    assert_eq!(
        call(
            &mut events,
            "events.subscribe",
            json!({"stream": "session/legacy", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("legacy")).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "missing"})).await["error"]["code"],
        -32602
    );
    let created = receive(&mut events).await;
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "legacy"})).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn host_lost_during_setup_answers_unavailable() {
    // Either the channel drops, or the client stops on its own full queue: the service ends.
    for target in ["host-lost-on-subscribe", "flood-on-subscribe"] {
        let (mut f, _) =
            refusing_fixture("", target, ads_ptyhost::wire::ErrorCode::InternalError, 0).await;
        let mut ws = authenticated(&f).await;
        transmit(&mut ws, &command("session.open", open_params("session"))).await;
        // The command itself stops the service, instead of a later turn of the loop.
        assert_eq!(
            bounded(&mut f.api).await.unwrap(),
            Err(ApiError::Unavailable),
            "{target}"
        );
        // The connection may close before the reply goes out; a reply that does says unavailable.
        if let Some(Ok(Message::Text(text))) = bounded(ws.next()).await {
            let reply: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(reply["error"]["code"], -32603, "{target}");
        }
    }
}

#[tokio::test]
async fn full_host_queue_during_setup_keeps_the_service() {
    let (f, calls) = refusing_fixture(
        "",
        "busy-on-open",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut ws = authenticated(&f).await;
    // The client refuses the subscription itself: setup failed, close scheduled, service alive.
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    wait_until(|| calls.lock().unwrap().len() >= 4).await;
    assert_eq!(
        &*calls.lock().unwrap(),
        &["hello", "open", "subscribe", "subscribe"]
    );
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

#[tokio::test]
async fn repeated_subscription_on_one_connection_gets_every_final_result() {
    let f = fixture("max_sessions = 1", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let subscribe = || {
        command(
            "events.subscribe",
            json!({"stream": "session/a", "after_seq": 0}),
        )
    };
    let (first, second) = (subscribe(), subscribe());
    for request in [&first, &second] {
        transmit(&mut ws, request).await;
        let reply = receive(&mut ws).await;
        assert_eq!(reply["id"], request["id"]);
        assert_eq!(reply["result"]["accepted"], true);
    }
    // Ids per connection are bounded too.
    let third = subscribe();
    transmit(&mut ws, &third).await;
    let refused = receive(&mut ws).await;
    assert_eq!(refused["id"], third["id"]);
    assert_eq!(refused["error"]["code"], -32003);
    call(&mut control, "session.open", open_params("a")).await;
    call(&mut control, "session.close", json!({"session_id": "a"})).await;
    // Another closed session prunes "a": every subscription gets its final result.
    call(&mut control, "session.open", open_params("b")).await;
    call(&mut control, "session.close", json!({"session_id": "b"})).await;
    let mut seqs = Vec::new();
    let mut ended = Vec::new();
    while ended.len() < 2 {
        let frame = receive(&mut ws).await;
        if let Some(seq) = frame["seq"].as_u64() {
            seqs.push(seq);
        } else {
            assert_eq!(frame["error"]["code"], -32004);
            ended.push(frame["id"].clone());
        }
    }
    // Each event went out once, and each id got its own end.
    assert!(!seqs.is_empty());
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
    assert!(ended.contains(&first["id"]) && ended.contains(&second["id"]));
}

#[tokio::test]
async fn repeated_subscription_with_a_lower_cursor_sends_events_again() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    call(&mut control, "session.open", open_params("a")).await;
    let resize = |cols: u64| json!({"session_id": "a", "cols": cols, "rows": 24});
    call(&mut control, "session.resize", resize(90)).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    assert_eq!(receive(&mut ws).await["seq"], 2);
    // The second subscription asks from the start: the shared cursor goes back and 1 and 2 go out again.
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    assert_eq!(receive(&mut ws).await["seq"], 2);
    call(&mut control, "session.resize", resize(100)).await;
    assert_eq!(receive(&mut ws).await["seq"], 3);
    // Nothing else is pending: the next frame answers this request.
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
}

#[tokio::test]
async fn ended_incarnation_answers_its_ids_before_the_new_subscription() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nretired_sessions = 1",
        "exit-then-hold-close",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut other = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let first = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &first).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    // The service waits for the host while the next commands queue behind the close.
    call(&mut control, "session.open", open_params("s")).await;
    let close = command("session.close", json!({"session_id": "s"}));
    transmit(&mut control, &close).await;
    wait_until(|| calls.lock().unwrap().iter().any(|c| c == "close")).await;
    let queued = f.hooks.queued_commands();
    // Queued first: a subscription that pushes the waiting "a" out of the quota.
    transmit(
        &mut other,
        &command(
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0}),
        ),
    )
    .await;
    wait_until(|| f.hooks.queued_commands() == queued + 1).await;
    // Queued second: the watcher subscribes "a" again and waits for the answer, without delivering.
    let second = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &second).await;
    wait_until(|| f.hooks.queued_commands() == queued + 2).await;
    f.release.notify_one();
    assert_eq!(receive(&mut control).await["id"], close["id"]);
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], first["id"]);
    assert_eq!(ended["error"]["code"], -32004);
    let confirmed = receive(&mut watcher).await;
    assert_eq!(confirmed["id"], second["id"]);
    assert_eq!(confirmed["result"]["accepted"], true);
}

async fn host_event_during_an_automatic_close(target: &'static str) {
    let (f, calls) = scripted_fixture(
        "max_sessions = 2\nhost_timeout_ms = 300\nclose_retry_ms = 20",
        target,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    call(&mut control, "session.open", open_params("a")).await;
    call(&mut control, "session.open", open_params("b")).await;
    call(
        &mut watcher,
        "events.subscribe",
        json!({"stream": "session/b", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    // "a" ends; the host holds the automatic close of "a" and reports the end of "b" meanwhile.
    call(
        &mut control,
        "session.write",
        json!({"session_id": "a", "data": "YQ=="}),
    )
    .await;
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["data"]["reason"], "exit");
    if target.ends_with("hold-close") {
        // The event came while the close of "a" still had no reply.
        assert_eq!(close_calls(&calls), 1);
        f.release.notify_one();
    }
    // After the reply, or after the deadline and one more attempt, both slots come back.
    let expected = if target.ends_with("hold-close") { 2 } else { 3 };
    wait_closes(&calls, expected).await;
    for id in ["c", "d"] {
        assert_eq!(
            call(&mut control, "session.open", open_params(id)).await["result"]["accepted"],
            true,
            "{target}"
        );
    }
    assert_eq!(close_calls(&calls), expected);
    assert_eq!(f.hooks.automatic_closes(), expected);
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn host_event_during_an_automatic_close_is_handled() {
    host_event_during_an_automatic_close("exit-other-then-hold-close").await;
}

#[tokio::test]
async fn host_event_during_a_timed_out_automatic_close_is_handled() {
    host_event_during_an_automatic_close("exit-other-then-drop-close").await;
}

/// A real shell that starts a second interpreter in a console of its own, then exits after 3 s.
/// The second one waits 60 s: closing the shell's console does not end it, only its Job Object does.
#[cfg(windows)]
struct LeftBehindLauncher;
#[cfg(windows)]
impl Launcher for LeftBehindLauncher {
    fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
        // Only what a system command needs to run; nothing else reaches the shell.
        let env: serde_json::Map<String, Value> = std::env::vars()
            .filter(|(name, _)| {
                ["SystemRoot", "windir", "ComSpec", "PATH", "PATHEXT"]
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(name))
            })
            .map(|(name, value)| (name, Value::String(value)))
            .collect();
        let system = r"%SystemRoot%\System32";
        let line = format!(
            r"start /min {system}\cmd.exe /d /c {system}\timeout.exe /t 60 /nobreak & {system}\timeout.exe /t 3 /nobreak >nul"
        );
        serde_json::from_value(json!({
            "session_id": request.session_id, "cols": request.cols, "rows": request.rows,
            "program": "cmd.exe", "args": ["/d", "/c", line], "cwd": null, "env": env
        }))
        .map_err(|_| ApiError::Invalid)
    }
}

#[cfg(windows)]
#[tokio::test]
async fn ended_session_leaves_no_child_behind_on_windows() {
    use ads_ptyhost::{
        PortablePtyBackend,
        win::{ProcessProbe, process_table},
    };
    let host =
        Arc::new(PtyHost::new(Arc::new(PortablePtyBackend), HostConfig::defaults()).unwrap());
    let (near, far) = tokio::io::duplex(262144);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = bounded(PtyClient::over(
        near,
        ClientConfig::recommended(host.config()),
    ))
    .await
    .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str("max_sessions = 1").unwrap(),
        client,
        Arc::new(LeftBehindLauncher),
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
    let f = Fixture {
        port: api.port().unwrap(),
        token,
        backend: Arc::new(FakeBackend::new()),
        host: host.clone(),
        hooks: api.test_hooks(),
        release: Arc::default(),
        api: tokio::spawn(api.run()),
        peer,
    };
    let mut ws = authenticated(&f).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/tree", "after_seq": 0}),
    )
    .await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("tree")).await["result"]["accepted"],
        true
    );
    let id = ads_ptyhost::wire::SessionId::new("tree").unwrap();
    // The shell is the session process whose parent is this test, which runs the host.
    let me = std::process::id();
    let children_of = |parent: u32| -> Vec<u32> {
        process_table()
            .unwrap()
            .into_iter()
            .filter(|entry| entry.parent_process_id == parent)
            .map(|entry| entry.process_id)
            .collect()
    };
    let mut shell = None;
    let mut left = Vec::new();
    bounded(async {
        while left.len() < 2 {
            let tree = host.tree_process_ids(&id).unwrap_or_default();
            shell = shell.or_else(|| {
                tree.iter()
                    .copied()
                    .find(|pid| children_of(me).contains(pid))
            });
            // Found by parent, not by the Job Object's list: the interpreter the shell started in a
            // console of its own (the shell's child that has children) and its 60 s wait.
            left = shell
                .into_iter()
                .flat_map(&children_of)
                .filter_map(|pid| {
                    let below = children_of(pid);
                    (!below.is_empty()).then(|| [vec![pid], below].concat())
                })
                .next()
                .unwrap_or_default();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    let tree = host.tree_process_ids(&id).unwrap();
    let probes: Vec<ProcessProbe> = left
        .iter()
        .map(|pid| ProcessProbe::open(*pid).expect("open a left-behind process"))
        .collect();
    for (pid, probe) in left.iter().zip(&probes) {
        assert!(probe.is_alive().expect("query a left-behind process"));
        assert!(tree.contains(pid), "{pid} outside the job: {tree:?}");
    }
    loop {
        let event = receive(&mut ws).await;
        if event["type"] == "urn:ads:event:session.ended:1" {
            assert_eq!(event["data"]["reason"], "exit");
            break;
        }
    }
    // The daemon gives the slot back on its own, and what ran in its own console is gone too.
    let alive = || {
        probes
            .iter()
            .any(|p| p.is_alive().expect("query a left-behind process"))
    };
    bounded(async {
        while !host.sessions().is_empty() || alive() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert_eq!(f.hooks.automatic_closes(), 1);
}
