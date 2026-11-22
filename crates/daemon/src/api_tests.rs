use super::*;

#[test]
fn the_approval_list_ceiling_stays_within_the_desktop_frame() {
    let ceiling = |bytes: usize| {
        ApiConfig::from_toml_str(&format!("approval_list_max_bytes = {bytes}"))
            .map(|config| config.approval_list_max_bytes)
    };
    assert_eq!(ceiling(8_388_608), Ok(8_388_608));
    assert_eq!(ceiling(1_048_576), Ok(1_048_576));
    assert!(ceiling(8_388_609).is_err());
    assert!(ceiling(268_435_456).is_err());
    assert!(ceiling(1_048_575).is_err());
    let default = ApiConfig::from_toml_str("")
        .unwrap()
        .approval_list_max_bytes;
    assert!(default <= MAX_APPROVAL_LIST_BYTES);
}

#[test]
fn malformed_credentials_never_escape_validation_errors() {
    let token = Token::generate().unwrap();
    let url = token.launch_url("http://localhost:5173", 1234);
    let secret = url.split_once("&token=").unwrap().1;
    let terminal = "private-terminal-output-sentinel";
    let value = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB", "method": "auth.hello", "params": {"token": secret, "terminal": terminal, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}}});
    let error = validate_command(value).unwrap_err();
    let diagnostics = format!("{error:?} {error} {token:?}");
    assert!(!diagnostics.contains(secret));
    assert!(!diagnostics.contains(terminal));
}
#[tokio::test]
async fn transient_accept_errors_retry_with_delay() {
    let mut attempts = 0;
    let start = Instant::now();
    let value = retry_io(
        || {
            attempts += 1;
            std::future::ready(if attempts < 3 {
                Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted))
            } else {
                Ok(42)
            })
        },
        10,
        &Gate::default(),
    )
    .await;
    assert_eq!(value, 42);
    assert_eq!(attempts, 3);
    assert!(start.elapsed() >= Duration::from_millis(20));
}

#[test]
fn hello_is_consumed_before_the_connection_loop() {
    let mut hello = Some(json!({"id": "request", "params": {"token": "credential-sentinel"}}));
    assert_eq!(consume_hello(&mut hello).unwrap(), "request");
    assert!(hello.is_none());
}
pub(crate) async fn fake_service() -> (
    LocalApi,
    Arc<ads_ptyhost::FakeBackend>,
    Arc<ads_ptyhost::PtyHost>,
    tokio::task::JoinHandle<()>,
) {
    use crate::pty_client::ClientConfig;
    use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
    struct FakeLauncher;
    impl Launcher for FakeLauncher {
        fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
            serde_json::from_value(json!({"session_id": request.session_id, "cols": request.cols, "rows": request.rows, "program": "fake", "args": [], "env": {}, "cwd": null})).map_err(|_| ApiError::Invalid)
        }
    }
    let backend = Arc::new(FakeBackend::new());
    let host = Arc::new(PtyHost::new(backend.clone(), HostConfig::defaults()).unwrap());
    let (near, far) = tokio::io::duplex(4096);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = PtyClient::over(near, ClientConfig::recommended(host.config()))
        .await
        .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str("max_sessions = 1").unwrap(),
        client,
        Arc::new(FakeLauncher),
    )
    .await
    .unwrap();
    (api, backend, host, peer)
}

#[tokio::test(start_paused = true)]
async fn confirmed_open_with_expired_setup_budget_schedules_cleanup() {
    let (mut api, backend, host, peer) = fake_service().await;
    let gate = api.service.before_setup.clone();
    gate.hold();
    let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
    let budget = Duration::from_secs(5);
    let deadline = Instant::now() + budget;
    let (result, ()) = timeout(Duration::from_secs(10), async {
        tokio::join!(
            api.service.execute("session.open", &params, None, deadline),
            async {
                gate.reached.notified().await;
                assert_eq!(host.sessions().len(), 1);
                tokio::time::advance(budget).await;
                gate.release();
            }
        )
    })
    .await
    .unwrap();
    assert_eq!(result.map(|_| ()), Err(ApiError::SetupFailed));
    // Let the host serve anything already sent: the subscription never reached it.
    tokio::time::sleep(Duration::from_millis(1)).await;
    let id: SessionId = "session".to_owned().try_into().unwrap();
    assert_eq!(host.subscriber_count(&id), Ok(0));
    assert_eq!(api.service.retries.len(), 1);
    let mut other = params.clone();
    other["session_id"] = json!("other");
    assert_eq!(
        api.service
            .execute("session.open", &other, None, Instant::now() + budget)
            .await
            .map(|_| ()),
        Err(ApiError::Capacity)
    );
    api.service.retry_close().await.unwrap();
    assert!(backend.terminals()[0].was_killed());
    assert!(host.sessions().is_empty());
    assert!(api.service.sessions.is_empty());
    assert!(
        api.service
            .execute("session.open", &params, None, Instant::now() + budget)
            .await
            .is_ok()
    );
    assert_eq!(host.sessions().len(), 1);
    assert!(api.service.retries.is_empty());
    peer.abort();
}

async fn open_fake(api: &mut LocalApi) -> SessionId {
    let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
    api.service
        .execute(
            "session.open",
            &params,
            None,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
    "session".to_owned().try_into().unwrap()
}
fn reschedule(service: &mut Service) {
    service.release_ended(&"session".to_owned().try_into().unwrap());
}

#[tokio::test]
async fn rescheduling_during_a_refused_attempt_keeps_one_entry_and_its_budget() {
    let (mut api, _, _, peer) = fake_service().await;
    let id = open_fake(&mut api).await;
    api.service.release_ended(&id);
    api.service.during_attempt = Some(|service| {
        reschedule(service);
        Some(ApiError::Host(ErrorCode::InternalError))
    });
    let before = Instant::now();
    api.service.retry_close().await.unwrap();
    assert_eq!(api.service.retries.len(), 1);
    assert_eq!(api.service.retries[0].attempts, 1);
    assert!(
        api.service.retries[0].due
            >= before + Duration::from_millis(api.service.config.close_retry_ms)
    );
    // The remaining budget runs out with no other entry left behind.
    for _ in 1..api.service.config.close_retries {
        api.service.retry_close().await.unwrap();
    }
    assert!(api.service.retries.is_empty());
    assert!(api.service.in_flight.is_none());
    peer.abort();
}

#[tokio::test]
async fn rescheduling_during_a_confirmed_attempt_returns_one_slot() {
    let (mut api, backend, host, peer) = fake_service().await;
    let id = open_fake(&mut api).await;
    api.service.release_ended(&id);
    api.service.during_attempt = Some(|service| {
        reschedule(service);
        None
    });
    api.service.retry_close().await.unwrap();
    assert!(backend.terminals()[0].was_killed());
    assert!(host.sessions().is_empty());
    assert!(api.service.retries.is_empty());
    assert_eq!(api.service.released.len(), 1);
    assert!(api.service.sessions["session"].released);
    peer.abort();
}

#[tokio::test]
async fn one_pending_close_per_generation() {
    let (mut api, _, _, peer) = fake_service().await;
    let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
    api.service
        .execute(
            "session.open",
            &params,
            None,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
    let id: SessionId = "session".to_owned().try_into().unwrap();
    api.service.release_ended(&id);
    api.service.retries[0].attempts = 2;
    api.service.release_ended(&id);
    api.service.invalidate(id).await;
    assert_eq!(api.service.retries.len(), 1);
    // The attempt already queued keeps its budget.
    assert_eq!(api.service.retries[0].attempts, 2);
    peer.abort();
}

#[tokio::test(start_paused = true)]
async fn ready_host_call_is_not_polled_at_the_deadline() {
    let (mut api, _, _, peer) = fake_service().await;
    let deadline = Instant::now() + Duration::from_secs(5);
    let ready = || std::future::ready(Ok::<_, ClientError>(()));
    assert_eq!(api.service.wait_host(ready(), deadline).await, Ok(()));
    tokio::time::advance(Duration::from_secs(5)).await;
    let mut polls = 0;
    for _ in 0..32 {
        let response = std::future::poll_fn(|_| {
            polls += 1;
            std::task::Poll::Ready(Ok::<_, ClientError>(()))
        });
        assert_eq!(
            api.service.wait_host(response, deadline).await,
            Err(ApiError::HostTimeout)
        );
    }
    assert_eq!(polls, 0);
    peer.abort();
}

/// A service whose host side the test writes by hand; `host.hello` is already answered.
async fn hand_driven_service() -> (LocalApi, tokio::io::DuplexStream) {
    use crate::pty_client::ClientConfig;
    use ads_ptyhost::wire::{self, ResultBody};
    use tokio::io::AsyncWriteExt;
    let (near, mut far) = tokio::io::duplex(65536);
    let hello = async {
        let wire::Frame::Json(payload) = wire::read_frame(&mut far, 131072).await.unwrap().unwrap()
        else {
            panic!()
        };
        let hello = wire::Request::parse(&payload).unwrap();
        far.write_all(
            &wire::encode_json(&wire::Response {
                id: Some(hello.id),
                outcome: Ok(ResultBody::Hello(wire::HelloResult {
                    contract: Default::default(),
                })),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    };
    let config = ClientConfig {
        max_frame_bytes: 131072,
        queue_capacity: 8,
    };
    let (client, ()) = tokio::join!(PtyClient::over(near, config), hello);
    let api = LocalApi::bind(
        ApiConfig::from_toml_str("").unwrap(),
        client.unwrap(),
        Arc::new(DisabledLauncher),
    )
    .await
    .unwrap();
    (api, far)
}

#[tokio::test(start_paused = true)]
async fn ready_reply_wins_over_a_deadline_crossed_by_a_host_event() {
    use tokio::io::AsyncWriteExt;
    let (mut api, mut far) = hand_driven_service().await;
    api.service.sessions.insert(
        "session".to_owned(),
        Session {
            generation: 1,
            offset: 0,
            last_host_seq: 0,
            ended: false,
            closing: false,
            invalidated: false,
            released: false,
            failed_open: false,
            self_released: false,
        },
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let (reply, answer) = oneshot::channel::<()>();
    let response = async { answer.await.map_err(|_| ClientError::Disconnected) };
    let log = api.service.log.clone();
    let (result, ()) = tokio::join!(api.service.wait_host(response, deadline), async {
        // The output handler waits for the log while the deadline passes and the reply arrives.
        let held = log.lock().await;
        let id = "session".to_owned().try_into().unwrap();
        far.write_all(&ads_ptyhost::wire::encode_output(&id, 1, b"terminal-sentinel").unwrap())
            .await
            .unwrap();
        // The clock only moves once every task waits: the handler is blocked on the log.
        tokio::time::sleep(Duration::from_millis(1)).await;
        tokio::time::advance(Duration::from_secs(5)).await;
        reply.send(()).unwrap();
        drop(held);
    });
    assert_eq!(result, Ok(()));
    assert_eq!(
        log.lock().await.after("session/session", 0).unwrap().len(),
        1
    );
}

#[tokio::test]
async fn failed_created_event_closes_the_opened_host_session() {
    let (mut api, backend, host, peer) = fake_service().await;
    api.service.log.lock().await.exhaust_sequence("session");
    let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
    assert_eq!(
        api.service
            .execute(
                "session.open",
                &params,
                None,
                Instant::now() + Duration::from_secs(5)
            )
            .await
            .map(|_| ()),
        Err(ApiError::SetupFailed)
    );
    // The record stays until the queued close is confirmed by the host.
    assert!(!backend.terminals()[0].was_killed());
    assert_eq!(api.service.sessions.len(), 1);
    api.service.retry_close().await.unwrap();
    assert!(backend.terminals()[0].was_killed());
    assert!(host.sessions().is_empty());
    assert!(api.service.sessions.is_empty());
    assert!(
        api.service
            .log
            .lock()
            .await
            .after("session/session", 0)
            .unwrap()
            .is_empty()
    );
    peer.abort();
}
#[tokio::test]
async fn bounded_host_queue_survives_a_paused_consumer() {
    use crate::pty_client::ClientConfig;
    use ads_ptyhost::wire::{self, Call, ResultBody};
    use tokio::io::AsyncWriteExt;
    let (near, mut far) = tokio::io::duplex(4096);
    let (begin, ready) = oneshot::channel();
    let peer = tokio::spawn(async move {
        let wire::Frame::Json(payload) = wire::read_frame(&mut far, 131072).await.unwrap().unwrap()
        else {
            panic!()
        };
        let hello = wire::Request::parse(&payload).unwrap();
        far.write_all(
            &wire::encode_json(&wire::Response {
                id: Some(hello.id),
                outcome: Ok(ResultBody::Hello(wire::HelloResult {
                    contract: Default::default(),
                })),
            })
            .unwrap(),
        )
        .await
        .unwrap();
        ready.await.unwrap();
        let id = "unknown".to_owned().try_into().unwrap();
        for seq in 1..=40 {
            far.write_all(&wire::encode_output(&id, seq, b"terminal-sentinel").unwrap())
                .await
                .unwrap();
        }
        let wire::Frame::Json(payload) = wire::read_frame(&mut far, 131072).await.unwrap().unwrap()
        else {
            panic!()
        };
        let request = wire::Request::parse(&payload).unwrap();
        assert!(matches!(request.call, Call::List(_)));
        far.write_all(
            &wire::encode_json(&wire::Response {
                id: Some(request.id),
                outcome: Ok(ResultBody::SessionList(wire::SessionList {
                    sessions: vec![],
                })),
            })
            .unwrap(),
        )
        .await
        .unwrap();
        std::future::pending::<()>().await;
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
    let mut api = LocalApi::bind(
        ApiConfig::from_toml_str("").unwrap(),
        client,
        Arc::new(DisabledLauncher),
    )
    .await
    .unwrap();
    begin.send(()).unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let host = api.service.host.commands();
    let result = timeout(
        Duration::from_secs(2),
        api.service
            .wait_host(host.list(), Instant::now() + Duration::from_secs(2)),
    )
    .await
    .unwrap();
    peer.abort();
    assert!(result.unwrap().sessions.is_empty());
}

pub(crate) async fn authenticate(
    port: u16,
    token: &str,
) -> tokio_tungstenite::WebSocketStream<TcpStream> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut request = format!("ws://127.0.0.1:{port}/")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Origin", "http://127.0.0.1:5173".parse().unwrap());
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let (mut ws, _) = tokio_tungstenite::client_async(request, stream)
        .await
        .unwrap();
    let hello = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB", "method": "auth.hello", "params": {"token": token, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}}});
    ws.send(Message::Text(hello.to_string().into()))
        .await
        .unwrap();
    let reply: Value =
        serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(reply["result"]["accepted"], true);
    ws
}

#[tokio::test]
async fn run_keeps_serving_while_accept_fails() {
    use crate::pty_client::ClientConfig;
    use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
    let host =
        Arc::new(PtyHost::new(Arc::new(FakeBackend::new()), HostConfig::defaults()).unwrap());
    let (near, far) = tokio::io::duplex(4096);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = PtyClient::over(near, ClientConfig::recommended(host.config()))
        .await
        .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str("accept_retry_ms = 20").unwrap(),
        client,
        Arc::new(DisabledLauncher),
    )
    .await
    .unwrap();
    let port = api.port().unwrap();
    let token = api
        .launch_url()
        .unwrap()
        .split_once("&token=")
        .unwrap()
        .1
        .to_owned();
    let hooks = api.test_hooks();
    let running = tokio::spawn(api.run());
    let mut first = timeout(Duration::from_secs(5), authenticate(port, &token))
        .await
        .unwrap();
    let start = Instant::now();
    hooks.accept_retry.hold();
    hooks.fail_accepts(25);
    // The accept already waiting takes this socket; the next ones hit the injected failures.
    let _armed = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let late = tokio::spawn({
        let token = token.clone();
        async move { authenticate(port, &token).await }
    });
    timeout(
        Duration::from_secs(5),
        hooks.accept_retry.reached.notified(),
    )
    .await
    .unwrap();
    assert_eq!(hooks.accept_faults_left(), 24);
    // The service answers while the next accept attempt is held.
    let subscribe = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAC", "method": "events.subscribe", "params": {"stream": "session/none", "after_seq": 0, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAC"}}});
    first
        .send(Message::Text(subscribe.to_string().into()))
        .await
        .unwrap();
    let reply = timeout(Duration::from_secs(5), first.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let reply: Value = serde_json::from_str(reply.to_text().unwrap()).unwrap();
    assert_eq!(reply["result"]["accepted"], true);
    assert_eq!(hooks.accept_faults_left(), 24);
    hooks.accept_retry.release();
    timeout(Duration::from_secs(5), late)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(hooks.accept_faults_left(), 0);
    assert!(start.elapsed() >= Duration::from_millis(500));
    assert!(!running.is_finished());
    running.abort();
    peer.abort();
}

#[tokio::test]
async fn evicted_connections_still_count_against_the_pending_cap() {
    use crate::pty_client::ClientConfig;
    use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
    let host =
        Arc::new(PtyHost::new(Arc::new(FakeBackend::new()), HostConfig::defaults()).unwrap());
    let (near, far) = tokio::io::duplex(4096);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = PtyClient::over(near, ClientConfig::recommended(host.config()))
        .await
        .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str("max_pending_auth = 1\nauth_timeout_ms = 60000").unwrap(),
        client,
        Arc::new(DisabledLauncher),
    )
    .await
    .unwrap();
    let port = api.port().unwrap();
    let hooks = api.test_hooks();
    let pending = api.pending.clone();
    let running = tokio::spawn(api.run());
    let wait_live = |count: usize| {
        let hooks = hooks.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while hooks.pending().0 != count {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap()
        }
    };
    let mut oldest = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    wait_live(1).await;
    // The evicted task is held before it ends, so its slot is still taken.
    hooks.evicted_exit.hold();
    let _newest = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    timeout(
        Duration::from_secs(5),
        hooks.evicted_exit.reached.notified(),
    )
    .await
    .unwrap();
    assert_eq!(pending.available_permits(), 0);
    assert_eq!(hooks.pending().0, 1);
    assert_eq!(hooks.pending().1, 1);
    hooks.evicted_exit.release();
    use tokio::io::AsyncReadExt;
    let mut byte = [0u8; 1];
    let read = timeout(Duration::from_secs(5), oldest.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(read, Ok(0) | Err(_)));
    wait_live(1).await;
    assert_eq!(hooks.pending().1, 1);
    running.abort();
    peer.abort();
}

#[tokio::test]
async fn invalidation_during_a_batch_suspends_the_remaining_frames() {
    let log = Arc::new(Mutex::new(EventLog::new(8, 4)));
    let stream = {
        let mut log = log.lock().await;
        for _ in 0..3 {
            log.append(
                "s",
                "resized",
                json!({"session_id": "s", "cols": 80, "rows": 24}),
                None,
            )
            .unwrap();
        }
        log.subscribe("session/s", 0).unwrap()
    };
    let Poll::Events(events) = stream.poll(0) else {
        panic!("expected events")
    };
    assert_eq!(events.len(), 3);
    let sent = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let (shared_log, shared_sent) = (log.clone(), sent.clone());
    // The first frame is still being sent when the session is invalidated.
    let mut sink = Box::pin(futures_util::sink::unfold(
        (),
        move |(), message: Message| {
            let (log, sent) = (shared_log.clone(), shared_sent.clone());
            async move {
                sent.lock()
                    .unwrap()
                    .push(message.to_text().unwrap().to_owned());
                log.lock().await.invalidate("s", Closing::Pending);
                Ok::<_, std::convert::Infallible>(())
            }
        },
    ));
    let mut after = 0;
    deliver(
        &mut sink,
        events,
        &mut after,
        &stream,
        &ApiConfig::from_toml_str("").unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert_eq!(after, 1);
    assert_eq!(stream.poll(after), Poll::Wait);
}
