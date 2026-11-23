use ads_contracts::CommandSessionOpenV1Params;
use ads_daemon::api::{ApiConfig, ApiError, Launcher, LocalApi, TestHooks};
use ads_daemon::events::new_id;
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server, wire::OpenParams};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use peer::{refusing_fixture, scripted_fixture};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

#[path = "local_api/auth.rs"]
mod auth;
#[path = "local_api/commands.rs"]
mod commands;
#[path = "local_api/config.rs"]
mod config;
#[path = "local_api/events.rs"]
mod events;
#[path = "local_api/host_failures.rs"]
mod host_failures;
#[path = "local_api/peer.rs"]
mod peer;
#[path = "local_api/session_cleanup.rs"]
mod session_cleanup;
#[path = "local_api/sessions.rs"]
mod sessions;
#[path = "local_api/tool_discovery.rs"]
mod tool_discovery;
#[path = "local_api/tool_launch.rs"]
mod tool_launch;
#[cfg(windows)]
#[path = "local_api/windows_sessions.rs"]
mod windows_sessions;

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
    fixture_with_config(
        ApiConfig::from_toml_str(overrides).unwrap(),
        launcher,
        host_config,
    )
    .await
}

async fn fixture_with_config(
    config: ApiConfig,
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
    let api = LocalApi::bind(config, client, launcher).await.unwrap();
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

fn remote(mut value: Value) -> Value {
    value["params"]["_meta"]["origin"] = json!("remote");
    value["params"]["_meta"]["remote"] = json!({"device_id": "other", "signature": STANDARD.encode([0;64]), "nonce": STANDARD.encode([0;16]), "counter": 1, "expires_at": "2030-01-01T00:00:00Z"});
    value
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

async fn wait_until(mut ready: impl FnMut() -> bool) {
    bounded(async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}
