//! The `pty-host/1` conversation on the fake terminal, wired straight to the connection handler.

mod common;

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use ads_ptyhost::wire::{
    self, Base64, Call, CloseReason, Closed, Empty, ErrorBody, ErrorCode, HelloParams, HelloResult,
    NoParams, Notification, OpenParams, ResizeParams, ResultBody, SessionEntry, SessionExited,
    SessionId, SessionParams, SubscriptionClosed, WriteParams,
};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, TerminalSize};
use common::client::{Client, Incoming, Transcript};
use tokio::io::DuplexStream;

const PIPE_BYTES: usize = 64 * 1024;

fn host_with(backend: &Arc<FakeBackend>, overrides: &str) -> Arc<PtyHost> {
    let config = HostConfig::from_toml_str(overrides).unwrap();
    Arc::new(PtyHost::new(Arc::clone(backend) as _, config).unwrap())
}

fn id(text: &str) -> SessionId {
    SessionId::new(text).unwrap()
}

fn seq(number: u64) -> NonZeroU64 {
    NonZeroU64::new(number).unwrap()
}

fn open(session: &str) -> Call {
    Call::Open(OpenParams {
        session_id: id(session),
        program: "fake-shell".to_owned().try_into().unwrap(),
        args: vec!["--flag".to_owned().try_into().unwrap()],
        cwd: None,
        env: BTreeMap::from([("LANG".to_owned(), "C".to_owned())]),
        cols: 120.try_into().unwrap(),
        rows: 40.try_into().unwrap(),
    })
}

fn close(session: &str) -> Call {
    Call::Close(SessionParams {
        session_id: id(session),
    })
}

fn list() -> Call {
    Call::List(NoParams {})
}

async fn greeted(host: &Arc<PtyHost>) -> Client<DuplexStream> {
    let mut client = Client::attach(host, PIPE_BYTES);
    client.hello().await;
    client
}

async fn sessions(client: &mut Client<DuplexStream>) -> Vec<SessionEntry> {
    match client.call(list()).await.unwrap() {
        ResultBody::SessionList(list) => list.sessions,
        other => panic!("unexpected list result: {other:?}"),
    }
}

fn code(outcome: Result<ResultBody, ErrorBody>) -> ErrorCode {
    outcome.expect_err("the call should have been refused").code
}

async fn output(client: &mut Client<DuplexStream>, transcript: &mut Transcript, session: &str) {
    match client.next().await {
        Some(Incoming::Output { header, bytes }) => {
            transcript.accept(&id(session), &header, &bytes)
        }
        other => panic!("expected output, got {other:?}"),
    }
}

#[tokio::test]
async fn hello_comes_first_and_negotiates_the_version() {
    let host = host_with(&Arc::new(FakeBackend::new()), "");
    let mut client = Client::attach(&host, PIPE_BYTES);

    assert_eq!(code(client.call(list()).await), ErrorCode::InvalidRequest);
    let other_version = Call::Hello(HelloParams {
        contract: "pty-host/2".to_owned(),
    });
    assert_eq!(
        code(client.call(other_version).await),
        ErrorCode::UnsupportedContract
    );
    // A refused hello does not open the door.
    assert_eq!(code(client.call(list()).await), ErrorCode::InvalidRequest);

    let hello = Call::Hello(HelloParams {
        contract: wire::CONTRACT.to_owned(),
    });
    assert_eq!(
        client.call(hello).await,
        Ok(ResultBody::Hello(HelloResult {
            contract: Default::default()
        }))
    );
    assert!(sessions(&mut client).await.is_empty());
}

#[tokio::test]
async fn session_is_opened_under_the_name_the_client_chose() {
    let backend = Arc::new(FakeBackend::with_echo());
    let host = host_with(&backend, "max_sessions = 2");
    let mut client = greeted(&host).await;

    assert_eq!(
        client.call(open("sess-a")).await,
        Ok(ResultBody::Empty(Empty {}))
    );
    assert_eq!(
        code(client.call(open("sess-a")).await),
        ErrorCode::SessionExists
    );

    // The terminal got exactly what the request carried.
    let terminal = backend.terminals()[0].clone();
    let spec = terminal.spec();
    assert_eq!(spec.program, "fake-shell");
    assert_eq!(spec.args, ["--flag"]);
    assert_eq!(spec.env, [("LANG".to_owned(), "C".to_owned())]);
    assert_eq!(
        spec.size,
        TerminalSize {
            rows: 40,
            cols: 120
        }
    );

    let bounds = client.subscribe(&id("sess-a"), None).await.unwrap();
    assert_eq!((bounds.oldest_seq, bounds.next_seq), (seq(1), seq(1)));
    let write = Call::Write(WriteParams {
        session_id: id("sess-a"),
        data: Base64(b"dir\r".to_vec()).into(),
    });
    assert_eq!(client.call(write).await, Ok(ResultBody::Empty(Empty {})));
    let mut transcript = Transcript::default();
    output(&mut client, &mut transcript, "sess-a").await;
    assert_eq!(transcript.bytes, b"dir\r");
    assert_eq!(terminal.input(), b"dir\r");

    let resize = Call::Resize(ResizeParams {
        session_id: id("sess-a"),
        cols: 80.try_into().unwrap(),
        rows: 24.try_into().unwrap(),
    });
    assert_eq!(client.call(resize).await, Ok(ResultBody::Empty(Empty {})));
    assert_eq!(terminal.size(), TerminalSize { rows: 24, cols: 80 });

    client.call(open("sess-b")).await.unwrap();
    let full = client.call(open("sess-c")).await.unwrap_err();
    assert_eq!(full.code, ErrorCode::TooManySessions);
    assert_eq!(full.data.unwrap().limit, Some(seq(2)));

    assert_eq!(
        sessions(&mut client).await,
        [
            SessionEntry {
                session_id: id("sess-a"),
                running: true,
                exit_code: None,
                oldest_seq: seq(1),
                next_seq: seq(2),
            },
            SessionEntry {
                session_id: id("sess-b"),
                running: true,
                exit_code: None,
                oldest_seq: seq(1),
                next_seq: seq(1),
            },
        ]
    );

    assert_eq!(
        client.call(close("sess-b")).await,
        Ok(ResultBody::Closed(Closed { exit_code: Some(1) }))
    );
    assert_eq!(
        code(client.call(close("sess-b")).await),
        ErrorCode::UnknownSession
    );
    assert_eq!(
        code(
            client
                .subscribe(&id("nobody"), None)
                .await
                .map(ResultBody::ReplayBounds)
        ),
        ErrorCode::UnknownSession
    );
}

#[tokio::test]
async fn replay_outside_the_ring_is_refused_with_the_bound() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(
        &backend,
        "replay_capacity_bytes = 16\nread_buffer_bytes = 8",
    );
    let mut client = greeted(&host).await;
    client.call(open("sess-a")).await.unwrap();
    let terminal = backend.terminals()[0].clone();
    for n in 0..10 {
        terminal.emit(format!("chunk{n:03}").as_bytes());
    }
    while sessions(&mut client).await[0].next_seq != seq(11) {
        tokio::task::yield_now().await;
    }

    let evicted = client.subscribe(&id("sess-a"), Some(3)).await.unwrap_err();
    assert_eq!(evicted.code, ErrorCode::ReplayEvicted);
    assert_eq!(evicted.data.unwrap().oldest_seq, Some(seq(9)));

    let ahead = client.subscribe(&id("sess-a"), Some(50)).await.unwrap_err();
    assert_eq!(ahead.code, ErrorCode::ReplayAhead);
    assert_eq!(ahead.data.unwrap().next_seq, Some(seq(11)));

    // Asking for everything gives what the ring still holds, and says where it starts.
    let bounds = client.subscribe(&id("sess-a"), None).await.unwrap();
    assert_eq!((bounds.oldest_seq, bounds.next_seq), (seq(9), seq(11)));
    let mut transcript = Transcript {
        last_seq: 8,
        ..Transcript::default()
    };
    output(&mut client, &mut transcript, "sess-a").await;
    output(&mut client, &mut transcript, "sess-a").await;
    assert_eq!(transcript.bytes, b"chunk008chunk009");
}

#[tokio::test]
async fn broken_requests_get_the_json_rpc_error_and_the_connection_survives() {
    let host = host_with(&Arc::new(FakeBackend::new()), "");
    let mut client = greeted(&host).await;

    let raw = |json: &str| {
        [
            &[0x01][..],
            &(json.len() as u32).to_be_bytes(),
            json.as_bytes(),
        ]
        .concat()
    };

    client.send_bytes(&raw("{not json")).await;
    let response = client.response().await;
    assert_eq!(response.id, None);
    assert_eq!(response.outcome.unwrap_err().code, ErrorCode::ParseError);

    client
        .send_bytes(&raw(
            r#"{"jsonrpc":"2.0","id":7,"method":"session.kill","params":{}}"#,
        ))
        .await;
    let response = client.response().await;
    assert_eq!(response.id, NonZeroU64::new(7));
    assert_eq!(
        response.outcome.unwrap_err().code,
        ErrorCode::MethodNotFound
    );

    client
        .send_bytes(&raw(
            r#"{"jsonrpc":"2.0","id":8,"method":"session.close","params":{"session_id":"a b"}}"#,
        ))
        .await;
    assert_eq!(
        client.response().await.outcome.unwrap_err().code,
        ErrorCode::InvalidParams
    );

    client
        .send_bytes(&raw(
            r#"{"jsonrpc":"2.0","id":0,"method":"session.list","params":{}}"#,
        ))
        .await;
    let response = client.response().await;
    assert_eq!(response.id, None);
    assert_eq!(
        response.outcome.unwrap_err().code,
        ErrorCode::InvalidRequest
    );

    assert!(sessions(&mut client).await.is_empty());
}

#[tokio::test]
async fn broken_framing_ends_the_connection_and_spares_the_sessions() {
    let host = host_with(&Arc::new(FakeBackend::new()), "max_frame_bytes = 4096");
    let mut keeper = greeted(&host).await;
    keeper.call(open("sess-a")).await.unwrap();

    let output_frame = wire::encode_output(&id("sess-a"), 1, b"x").unwrap();
    let breakers: [&[u8]; 3] = [
        // A kind that does not exist, a frame above the limit, and output sent by a client.
        &[0x7f, 0, 0, 0, 0],
        &[0x01, 0, 0, 0x10, 0x01],
        &output_frame,
    ];
    for breaker in breakers {
        let mut client = greeted(&host).await;
        client.send_bytes(breaker).await;
        assert_eq!(client.next().await, None, "{breaker:?}");
    }

    assert_eq!(sessions(&mut keeper).await.len(), 1);
}

#[tokio::test]
async fn exit_is_announced_after_the_last_output() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let mut client = greeted(&host).await;
    client.call(open("sess-a")).await.unwrap();
    client.subscribe(&id("sess-a"), Some(1)).await.unwrap();

    let terminal = backend.terminals()[0].clone();
    terminal.emit(b"one");
    terminal.emit(b"two");
    terminal.exit(7);

    let mut transcript = Transcript::default();
    output(&mut client, &mut transcript, "sess-a").await;
    output(&mut client, &mut transcript, "sess-a").await;
    assert_eq!(
        client.next().await,
        Some(Incoming::Notification(Notification::SessionExited(
            SessionExited {
                session_id: id("sess-a"),
                exit_code: Some(7),
                last_seq: 2,
            }
        )))
    );

    // The session stays listed, ended, with its replay, until someone closes it.
    let listed = sessions(&mut client).await;
    assert_eq!((listed[0].running, listed[0].exit_code), (false, Some(7)));
    client.subscribe(&id("sess-a"), Some(2)).await.unwrap();
    let mut late = Transcript {
        last_seq: 1,
        ..Transcript::default()
    };
    output(&mut client, &mut late, "sess-a").await;
    assert_eq!(late.bytes, b"two");
    assert_eq!(
        client.call(close("sess-a")).await,
        Ok(ResultBody::Closed(Closed { exit_code: Some(7) }))
    );
}

#[tokio::test]
async fn closing_a_session_tells_its_subscribers() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let mut watcher = greeted(&host).await;
    let mut other = greeted(&host).await;
    other.call(open("sess-a")).await.unwrap();
    watcher.subscribe(&id("sess-a"), None).await.unwrap();

    backend.terminals()[0].emit(b"last words");
    let mut transcript = Transcript::default();
    output(&mut watcher, &mut transcript, "sess-a").await;
    other.call(close("sess-a")).await.unwrap();

    assert_eq!(
        watcher.next().await,
        Some(Incoming::Notification(Notification::SubscriptionClosed(
            SubscriptionClosed {
                session_id: id("sess-a"),
                reason: CloseReason::SessionClosed,
                last_seq: 1,
            }
        )))
    );
}

#[tokio::test]
async fn slow_client_is_unsubscribed_and_comes_back_through_replay() {
    const CHUNKS: u64 = 200;
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "subscriber_queue_chunks = 2");
    // A small pipe: the host can only write a few frames before this client has to read.
    let mut slow = Client::attach(&host, 4096);
    slow.hello().await;
    slow.call(open("sess-a")).await.unwrap();
    slow.subscribe(&id("sess-a"), None).await.unwrap();

    let terminal = backend.terminals()[0].clone();
    let mut emitted = Vec::new();
    for n in 0..CHUNKS {
        let chunk = vec![b'a' + (n % 26) as u8; 1000];
        terminal.emit(&chunk);
        emitted.extend(chunk);
    }
    // Another client watches the session reach the end while the slow one reads nothing.
    let mut observer = greeted(&host).await;
    while sessions(&mut observer).await[0].next_seq != seq(CHUNKS + 1) {
        tokio::task::yield_now().await;
    }

    let mut transcript = Transcript::default();
    let closed = loop {
        match slow.next().await.expect("channel closed") {
            Incoming::Output { header, bytes } => transcript.accept(&id("sess-a"), &header, &bytes),
            Incoming::Notification(notification) => break notification,
        }
    };
    assert_eq!(
        closed,
        Notification::SubscriptionClosed(SubscriptionClosed {
            session_id: id("sess-a"),
            reason: CloseReason::SlowSubscriber,
            last_seq: transcript.last_seq,
        })
    );
    assert!(
        transcript.last_seq < CHUNKS,
        "the slow client was never dropped"
    );

    slow.subscribe(&id("sess-a"), Some(transcript.last_seq + 1))
        .await
        .unwrap();
    while transcript.last_seq < CHUNKS {
        output(&mut slow, &mut transcript, "sess-a").await;
    }
    assert_eq!(transcript.bytes, emitted);
}

#[tokio::test]
async fn a_new_subscription_replaces_the_previous_one_of_the_same_client() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let mut client = greeted(&host).await;
    client.call(open("sess-a")).await.unwrap();
    let terminal = backend.terminals()[0].clone();
    terminal.emit(b"a");
    terminal.emit(b"b");

    client.subscribe(&id("sess-a"), None).await.unwrap();
    let mut first = Transcript::default();
    output(&mut client, &mut first, "sess-a").await;
    output(&mut client, &mut first, "sess-a").await;

    // Subscribing again from 2 delivers 2 once more and then only the new subscription is live:
    // "c" arrives a single time.
    client.subscribe(&id("sess-a"), Some(2)).await.unwrap();
    terminal.emit(b"c");
    let mut second = Transcript {
        last_seq: 1,
        ..Transcript::default()
    };
    output(&mut client, &mut second, "sess-a").await;
    output(&mut client, &mut second, "sess-a").await;
    assert_eq!(second.bytes, b"bc");
    terminal.exit(0);
    assert!(matches!(
        client.next().await,
        Some(Incoming::Notification(Notification::SessionExited(_)))
    ));
}

/// Waits until the session holds exactly `count` subscriptions.
async fn wait_for_subscribers(host: &PtyHost, session: &str, count: usize) {
    let settled = tokio::time::timeout(common::STEP_TIMEOUT, async {
        while host.subscriber_count(&id(session)).unwrap() != count {
            tokio::task::yield_now().await;
        }
    });
    if settled.await.is_err() {
        panic!(
            "expected {count} subscriptions, the session still holds {}",
            host.subscriber_count(&id(session)).unwrap()
        );
    }
}

#[tokio::test]
async fn clients_that_leave_or_subscribe_again_leave_nothing_behind_on_an_idle_session() {
    const CYCLES: usize = 200;
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let mut owner = greeted(&host).await;
    owner.call(open("sess-a")).await.unwrap();

    // Clients that subscribe and drop the connection.
    for _ in 0..CYCLES {
        let mut leaver = greeted(&host).await;
        leaver.subscribe(&id("sess-a"), None).await.unwrap();
    }
    wait_for_subscribers(&host, "sess-a", 0).await;

    // One client subscribing over and over: only its latest subscription stays.
    for _ in 0..CYCLES {
        owner.subscribe(&id("sess-a"), None).await.unwrap();
    }
    wait_for_subscribers(&host, "sess-a", 1).await;
    drop(owner);
    wait_for_subscribers(&host, "sess-a", 0).await;

    // The terminal never printed anything.
    let mut observer = greeted(&host).await;
    assert_eq!(sessions(&mut observer).await[0].next_seq, seq(1));
}
