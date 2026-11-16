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
    let backend = Arc::new(FakeBackend::with_echo());
    let host = Arc::new(PtyHost::new(backend.clone(), HostConfig::defaults()).unwrap());
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
    assert_eq!(http, "http://localhost:5173/");
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
    let mut ws = connect(f, Some("http://localhost:5173"), None)
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
        (Some("http://localhost:5173"), Some("untrusted.invalid")),
        (Some("null"), None),
        (Some("http://localhost:5173.attacker.invalid"), None),
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
    for origin in ["http://localhost:5173", "http://127.0.0.1:5173"] {
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
        let mut ws = connect(&f, Some("http://localhost:5173"), None)
            .await
            .unwrap();
        transmit(&mut ws, &value).await;
        closed(&mut ws).await;
    }
    let mut ws = connect(&f, Some("http://localhost:5173"), None)
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
    let url = token.launch_url(12345);
    let (http, fragment) = url.split_once('#').unwrap();
    let secret = fragment.split_once("&token=").unwrap().1;
    assert_eq!(http, "http://localhost:5173/");
    assert!(!http.contains(secret));
    assert!(token.matches(secret));
    assert!(!token.matches(&"X".repeat(43)));
    assert!(!token.matches(""));
    assert_ne!(Token::generate().unwrap().launch_url(12345), url);
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
    assert!(
        connect(&f, Some("http://localhost:5173"), None)
            .await
            .is_err()
    );
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
        ("/", "Origin: http://localhost:5173\r\n".to_owned()),
        (
            "/",
            format!(
                "Host: 127.0.0.1:{}\r\nOrigin: http://localhost:5173\r\nOrigin: http://localhost:5173\r\n",
                f.port
            ),
        ),
        (
            "/",
            format!(
                "Host: 127.0.0.1:{}\r\nHost: localhost:{}\r\nOrigin: http://localhost:5173\r\n",
                f.port, f.port
            ),
        ),
        (
            "/?token=refused",
            format!(
                "Host: 127.0.0.1:{}\r\nOrigin: http://localhost:5173\r\n",
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
