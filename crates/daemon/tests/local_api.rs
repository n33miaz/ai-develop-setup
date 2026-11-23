use ads_contracts::CommandSessionOpenV1Params;
use ads_daemon::api::{ApiConfig, ApiError, Launcher, LocalApi, TestHooks};
use ads_daemon::events::new_id;
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
