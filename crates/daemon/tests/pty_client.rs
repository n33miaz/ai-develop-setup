use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use ads_daemon::pty_client::{ClientConfig, ClientError, Event, PtyClient};
use ads_ptyhost::wire::{
    self, Base64, ErrorCode, Frame, Notification, OpenParams, ResizeParams, SessionId,
    SessionParams, WriteParams,
};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};

fn id() -> SessionId {
    SessionId::new("client-test").unwrap()
}
fn open() -> OpenParams {
    OpenParams {
        session_id: id(),
        program: "fake-shell".to_owned().try_into().unwrap(),
        args: vec![],
        cwd: None,
        env: BTreeMap::new(),
        cols: 80.try_into().unwrap(),
        rows: 24.try_into().unwrap(),
    }
}
async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .expect("test timed out")
}
fn host(overrides: &str) -> (Arc<FakeBackend>, Arc<PtyHost>) {
    let backend = Arc::new(FakeBackend::with_echo());
    let host = Arc::new(
        PtyHost::new(
            backend.clone(),
            HostConfig::from_toml_str(overrides).unwrap(),
        )
        .unwrap(),
    );
    (backend, host)
}
async fn attach(host: &Arc<PtyHost>) -> PtyClient {
    let (near, far) = tokio::io::duplex(4096);
    tokio::spawn(server::connection(host.clone(), far));
    bounded(PtyClient::over(
        near,
        ClientConfig::recommended(host.config()),
    ))
    .await
    .unwrap()
}
async fn output(client: &mut PtyClient, seq: u64, expected: &[u8]) {
    match bounded(client.next()).await.unwrap() {
        Event::Output { header, bytes } => {
            assert_eq!(header.session_id, id());
            assert_eq!(header.seq.get(), seq);
            assert_eq!(bytes, expected);
        }
        other => panic!("expected output: {other:?}"),
    }
}
fn code(error: ClientError) -> i64 {
    match error {
        ClientError::Host { code, .. } => code.number(),
        other => panic!("expected host error: {other}"),
    }
}

#[tokio::test]
async fn hello_session_operations_and_replay_use_the_host_contract() {
    let (backend, host) = host("");
    let mut client = attach(&host).await;
    assert!(client.list().await.unwrap().sessions.is_empty());
    client.open(open()).await.unwrap();
    assert_eq!(code(client.open(open()).await.unwrap_err()), -32002);
    assert_eq!(client.list().await.unwrap().sessions[0].session_id, id());
    client.subscribe(id(), Some(0)).await.unwrap();
    for (n, bytes) in [b"first".as_slice(), b"second"].into_iter().enumerate() {
        client
            .write(WriteParams {
                session_id: id(),
                data: Base64(bytes.to_vec()).into(),
            })
            .await
            .unwrap();
        output(&mut client, n as u64 + 1, bytes).await;
    }
    client
        .resize(ResizeParams {
            session_id: id(),
            cols: 101.try_into().unwrap(),
            rows: 37.try_into().unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(
        backend.terminals()[0].size(),
        ads_ptyhost::TerminalSize {
            rows: 37,
            cols: 101
        }
    );
    assert_eq!(backend.terminals()[0].input(), b"firstsecond");
    drop(client);
    let mut client = attach(&host).await;
    let bounds = client.subscribe(id(), Some(1)).await.unwrap();
    assert_eq!(bounds.next_seq.get(), 3);
    output(&mut client, 2, b"second").await;
    assert_eq!(
        code(client.subscribe(id(), Some(99)).await.unwrap_err()),
        -32005
    );
    assert_eq!(
        client.subscribe(id(), Some(u64::MAX)).await.unwrap_err(),
        ClientError::SequenceOverflow
    );
    assert_eq!(
        client
            .close(SessionParams { session_id: id() })
            .await
            .unwrap()
            .exit_code,
        Some(1)
    );
    assert!(client.list().await.unwrap().sessions.is_empty());
    assert_eq!(
        code(
            client
                .write(WriteParams {
                    session_id: id(),
                    data: Base64(vec![]).into()
                })
                .await
                .unwrap_err()
        ),
        -32001
    );
}

#[tokio::test]
async fn evicted_replay_preserves_the_code_and_bound() {
    let (backend, host) = host("replay_capacity_bytes = 4\nread_buffer_bytes = 4");
    let mut client = attach(&host).await;
    client.open(open()).await.unwrap();
    client.subscribe(id(), None).await.unwrap();
    backend.terminals()[0].emit(b"aaaa");
    output(&mut client, 1, b"aaaa").await;
    backend.terminals()[0].emit(b"bbbb");
    output(&mut client, 2, b"bbbb").await;
    match client.subscribe(id(), Some(0)).await.unwrap_err() {
        ClientError::Host {
            code: ErrorCode::ReplayEvicted,
            data: Some(data),
        } => assert_eq!(data.oldest_seq.unwrap().get(), 2),
        other => panic!("wrong replay error: {other}"),
    }
    client
        .close(SessionParams { session_id: id() })
        .await
        .unwrap();
}

struct Gate {
    paused: std::sync::atomic::AtomicBool,
    waker: std::sync::Mutex<Option<std::task::Waker>>,
    entered: tokio::sync::Notify,
}
impl Gate {
    fn release(&self) {
        self.paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}
struct GatedRead {
    inner: tokio::io::DuplexStream,
    gate: Arc<Gate>,
}
impl tokio::io::AsyncRead for GatedRead {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let mut waker = self.gate.waker.lock().unwrap();
        if self.gate.paused.load(std::sync::atomic::Ordering::SeqCst) {
            *waker = Some(cx.waker().clone());
            self.gate.entered.notify_one();
            return std::task::Poll::Pending;
        }
        drop(waker);
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
impl tokio::io::AsyncWrite for GatedRead {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[tokio::test]
async fn slow_subscription_resumes_after_last_delivered_sequence() {
    let (backend, host) = host("subscriber_queue_chunks = 1\nread_buffer_bytes = 1");
    let (near, far) = tokio::io::duplex(1);
    tokio::spawn(server::connection(host.clone(), far));
    let gate = Arc::new(Gate {
        paused: false.into(),
        waker: Default::default(),
        entered: Default::default(),
    });
    let mut client = PtyClient::over(
        GatedRead {
            inner: near,
            gate: gate.clone(),
        },
        ClientConfig {
            max_frame_bytes: host.config().max_frame_bytes,
            queue_capacity: 128,
        },
    )
    .await
    .unwrap();
    client.open(open()).await.unwrap();
    client.subscribe(id(), None).await.unwrap();
    gate.paused.store(true, std::sync::atomic::Ordering::SeqCst);
    backend.terminals()[0].emit(b"a");
    bounded(gate.entered.notified()).await;
    backend.terminals()[0].emit(b"bcdefghij");
    bounded(async {
        while host.sessions()[0].next_seq.0 != 11 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert_eq!(host.subscriber_count(&id()).unwrap(), 0);
    gate.release();
    let mut last = 0;
    let mut transcript = Vec::new();
    let mut closed = false;
    while last < 10 {
        match bounded(client.next()).await.unwrap() {
            Event::Output { header, bytes } => {
                assert_eq!(header.seq.get(), last + 1);
                last += 1;
                transcript.extend(bytes);
            }
            Event::Notification(Notification::SubscriptionClosed(params)) => {
                assert_eq!(params.last_seq, last);
                closed = true;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert!(closed);
    assert_eq!(transcript, b"abcdefghij");
    client
        .close(SessionParams { session_id: id() })
        .await
        .unwrap();
}

#[tokio::test]
async fn errors_never_include_the_host_message_or_environment() {
    use tokio::io::AsyncWriteExt;
    let (near, mut far) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        let wire::Frame::Json(payload) = wire::read_frame(&mut far, 4096).await.unwrap().unwrap()
        else {
            panic!()
        };
        let request = wire::Request::parse(&payload).unwrap();
        let response = wire::Response {
            id: Some(request.id),
            outcome: Err(wire::ErrorBody::new(
                ErrorCode::SpawnFailed,
                "secret-terminal-and-environment",
                None,
            )),
        };
        far.write_all(&wire::encode_json(&response).unwrap())
            .await
            .unwrap();
    });
    let error = match PtyClient::over(
        near,
        ClientConfig {
            max_frame_bytes: 4096,
            queue_capacity: 2,
        },
    )
    .await
    {
        Ok(_) => panic!("hello accepted"),
        Err(error) => error,
    };
    assert_eq!(code(error.clone()), -32007);
    assert!(!format!("{error:?} {error}").contains("secret-terminal-and-environment"));
}

#[tokio::test]
async fn cancelling_next_keeps_a_partial_frame_intact() {
    use tokio::io::AsyncWriteExt;
    let (near, mut far) = tokio::io::duplex(4096);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let Frame::Json(payload) = wire::read_frame(&mut far, 4096).await.unwrap().unwrap() else {
            panic!()
        };
        let request = wire::Request::parse(&payload).unwrap();
        let response = wire::Response {
            id: Some(request.id),
            outcome: Ok(wire::ResultBody::Hello(wire::HelloResult {
                contract: Default::default(),
            })),
        };
        far.write_all(&wire::encode_json(&response).unwrap())
            .await
            .unwrap();
        let frame = wire::encode_output(&id(), 1, b"partial-output").unwrap();
        let split = frame.len() / 2;
        far.write_all(&frame[..split]).await.unwrap();
        ready_tx.send(()).unwrap();
        finish_rx.await.unwrap();
        far.write_all(&frame[split..]).await.unwrap();
    });
    let mut client = PtyClient::over(
        near,
        ClientConfig {
            max_frame_bytes: 4096,
            queue_capacity: 2,
        },
    )
    .await
    .unwrap();
    ready_rx.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), client.next())
            .await
            .is_err()
    );
    finish_tx.send(()).unwrap();
    output(&mut client, 1, b"partial-output").await;
    bounded(peer).await.unwrap();
}

#[tokio::test]
async fn event_queue_overflow_is_explicit_and_keeps_consumed_sequence_usable() {
    let (backend, host) = host("");
    let (near, far) = tokio::io::duplex(4096);
    tokio::spawn(server::connection(host.clone(), far));
    let mut client = PtyClient::over(
        near,
        ClientConfig {
            max_frame_bytes: 4096,
            queue_capacity: 1,
        },
    )
    .await
    .unwrap();
    client.open(open()).await.unwrap();
    client.subscribe(id(), None).await.unwrap();
    backend.terminals()[0].emit(b"one");
    // A round trip ensures the first frame is queued before emitting the second.
    bounded(async {
        while host.sessions()[0].next_seq.0 != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    client.list().await.unwrap();
    backend.terminals()[0].emit(b"two");
    bounded(async {
        while host.sessions()[0].next_seq.0 != 3 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    // Wait for the worker to observe the full queue, without consuming its first event.
    bounded(async {
        loop {
            if client.list().await == Err(ClientError::Capacity) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    output(&mut client, 1, b"one").await;
    assert_eq!(client.next().await.unwrap_err(), ClientError::Capacity);
    drop(client);
    let mut client = attach(&host).await;
    client.subscribe(id(), Some(1)).await.unwrap();
    output(&mut client, 2, b"two").await;
    client
        .close(SessionParams { session_id: id() })
        .await
        .unwrap();
}

#[tokio::test]
async fn removed_session_reports_the_failed_automatic_resubscribe() {
    let (_backend, host) = host("");
    let mut client = attach(&host).await;
    client.open(open()).await.unwrap();
    client.subscribe(id(), None).await.unwrap();
    host.close(&id()).await.unwrap();
    assert!(matches!(
        bounded(client.next()).await.unwrap(),
        Event::Notification(Notification::SubscriptionClosed(_))
    ));
    match bounded(client.next()).await.unwrap() {
        Event::ResubscribeFailed { session_id, error } => {
            assert_eq!(session_id, id());
            assert_eq!(code(error), -32001);
        }
        other => panic!("wrong recovery event: {other:?}"),
    }
    assert!(client.list().await.unwrap().sessions.is_empty());
}

#[tokio::test]
async fn pending_writes_do_not_block_receiving_host_frames() {
    use tokio::io::AsyncWriteExt;
    let (near, mut far) = tokio::io::duplex(16);
    let (began_tx, began_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let Frame::Json(payload) = wire::read_frame(&mut far, 16384).await.unwrap().unwrap() else {
            panic!()
        };
        let hello = wire::Request::parse(&payload).unwrap();
        far.write_all(
            &wire::encode_json(&wire::Response {
                id: Some(hello.id),
                outcome: Ok(wire::ResultBody::Hello(wire::HelloResult {
                    contract: Default::default(),
                })),
            })
            .unwrap(),
        )
        .await
        .unwrap();
        began_rx.await.unwrap();
        // The peer deliberately finishes all output before reading the large request.
        for seq in 1..=8 {
            far.write_all(&wire::encode_output(&id(), seq, &[b'x'; 1024]).unwrap())
                .await
                .unwrap();
        }
        let Frame::Json(payload) = wire::read_frame(&mut far, 16384).await.unwrap().unwrap() else {
            panic!()
        };
        let request = wire::Request::parse(&payload).unwrap();
        assert!(matches!(request.call, wire::Call::Write(_)));
        far.write_all(
            &wire::encode_json(&wire::Response {
                id: Some(request.id),
                outcome: Ok(wire::ResultBody::Empty(wire::Empty {})),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    });
    let mut client = PtyClient::over(
        near,
        ClientConfig {
            max_frame_bytes: 16384,
            queue_capacity: 16,
        },
    )
    .await
    .unwrap();
    let params = WriteParams {
        session_id: id(),
        data: Base64(vec![b'w'; 4096]).into(),
    };
    began_tx.send(()).unwrap();
    bounded(client.write(params)).await.unwrap();
    for seq in 1..=8 {
        output(&mut client, seq, &[b'x'; 1024]).await;
    }
    bounded(peer).await.unwrap();
}
