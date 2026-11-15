//! A terminal call that takes long holds back only the client that asked for it. Every wait here
//! is a gate the test opens itself: the order of events is proven, not guessed from a sleep.

mod common;

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ads_ptyhost::wire::{
    Base64, Call, ErrorBody, ErrorCode, NoParams, OpenParams, ResizeParams, ResultBody, SessionId,
    SessionParams, WriteParams,
};
use ads_ptyhost::{
    FakeBackend, HostConfig, PtyError, PtyHost, ReplayFrom, SpawnSpec, TerminalSize,
};
use common::STEP_TIMEOUT;
use common::client::{Client, Incoming};
use common::gated::{GatedBackend, Gates, Op, PANIC_PROGRAM};
use tokio::io::DuplexStream;
use tokio::sync::mpsc::UnboundedReceiver;

const PIPE_BYTES: usize = 64 * 1024;

struct Fixture {
    host: Arc<PtyHost>,
    fake: Arc<FakeBackend>,
    gates: Arc<Gates>,
    arrivals: UnboundedReceiver<Op>,
}

fn fixture(overrides: &str) -> Fixture {
    let fake = Arc::new(FakeBackend::new());
    let (gates, arrivals) = Gates::new();
    let backend = GatedBackend {
        inner: Arc::clone(&fake),
        gates: Arc::clone(&gates),
    };
    let config = HostConfig::from_toml_str(overrides).unwrap();
    Fixture {
        host: Arc::new(PtyHost::new(Arc::new(backend), config).unwrap()),
        fake,
        gates,
        arrivals,
    }
}

impl Fixture {
    /// Waits until a held call of this kind is stopped at its gate.
    async fn arrival(&mut self, op: Op) {
        let arrived = tokio::time::timeout(STEP_TIMEOUT, self.arrivals.recv())
            .await
            .expect("the held call never reached the terminal");
        assert_eq!(arrived, Some(op));
    }

    /// The held call is still stopped at its gate, and no gate opened by itself.
    fn assert_still_held(&self, op: Op) {
        assert!(
            self.gates.is_inside(op) && !self.gates.gave_up(),
            "the host answered only after the held {op:?} went through"
        );
    }

    async fn greeted(&self) -> Client<DuplexStream> {
        let mut client = Client::attach(&self.host, PIPE_BYTES);
        client.hello().await;
        client
    }
}

fn id(text: &str) -> SessionId {
    SessionId::new(text).unwrap()
}

fn spec() -> SpawnSpec {
    SpawnSpec {
        program: "fake-shell".to_owned(),
        ..SpawnSpec::default()
    }
}

fn open(session: &str) -> Call {
    Call::Open(OpenParams {
        session_id: id(session),
        program: "fake-shell".to_owned(),
        args: Vec::new(),
        cwd: None,
        env: BTreeMap::new(),
        cols: 80.try_into().unwrap(),
        rows: 24.try_into().unwrap(),
    })
}

fn close(session: &str) -> Call {
    Call::Close(SessionParams {
        session_id: id(session),
    })
}

fn code(outcome: Result<ResultBody, ErrorBody>) -> ErrorCode {
    outcome.expect_err("the call should have been refused").code
}

async fn names(client: &mut Client<DuplexStream>) -> Vec<String> {
    match client.call(Call::List(NoParams {})).await.unwrap() {
        ResultBody::SessionList(list) => list
            .sessions
            .iter()
            .map(|entry| entry.session_id.as_str().to_owned())
            .collect(),
        other => panic!("unexpected list result: {other:?}"),
    }
}

async fn output(client: &mut Client<DuplexStream>) -> Vec<u8> {
    match client.next().await {
        Some(Incoming::Output { bytes, .. }) => bytes,
        other => panic!("expected output, got {other:?}"),
    }
}

/// What every other client must still get while one call is held: a new connection is greeted,
/// the list is answered and live output of an open session keeps flowing.
async fn others_are_served(
    fixture: &Fixture,
    watcher: &mut Client<DuplexStream>,
    expected: &[&str],
) -> Duration {
    let started = Instant::now();
    let mut newcomer = fixture.greeted().await;
    assert_eq!(names(&mut newcomer).await, expected);
    fixture.fake.terminals()[0].emit(b"still flowing");
    assert_eq!(output(watcher).await, b"still flowing");
    started.elapsed()
}

#[tokio::test]
async fn a_slow_spawn_holds_back_only_the_client_that_asked() {
    let mut fixture = fixture("");
    let mut watcher = fixture.greeted().await;
    watcher.call(open("live")).await.unwrap();
    watcher.subscribe(&id("live"), None).await.unwrap();

    fixture.gates.hold(Op::Spawn);
    let mut opener = fixture.greeted().await;
    let asked = opener.request(open("slow")).await;
    fixture.arrival(Op::Spawn).await;

    let served_in = others_are_served(&fixture, &mut watcher, &["live"]).await;
    // The name is taken from the start, but the session is not there to be used yet.
    let mut other = fixture.greeted().await;
    assert_eq!(
        code(other.call(open("slow")).await),
        ErrorCode::SessionExists
    );
    assert_eq!(
        code(
            other
                .subscribe(&id("slow"), None)
                .await
                .map(|_| unreachable!())
        ),
        ErrorCode::UnknownSession
    );
    assert_eq!(
        code(other.call(close("slow")).await),
        ErrorCode::UnknownSession
    );
    fixture.assert_still_held(Op::Spawn);
    println!("spawn held: hello, list and live output served in {served_in:?}");

    fixture.gates.release(Op::Spawn);
    let response = opener.response().await;
    assert_eq!(response.id, Some(asked));
    response.outcome.unwrap();
    assert_eq!(names(&mut other).await, ["live", "slow"]);
    assert_eq!(
        code(other.call(open("slow")).await),
        ErrorCode::SessionExists
    );
}

#[tokio::test]
async fn slow_resize_write_and_close_hold_back_only_the_client_that_asked() {
    let mut fixture = fixture("");
    let mut watcher = fixture.greeted().await;
    watcher.call(open("live")).await.unwrap();
    watcher.call(open("busy")).await.unwrap();
    watcher.subscribe(&id("live"), None).await.unwrap();

    let calls = [
        (
            Op::Resize,
            Call::Resize(ResizeParams {
                session_id: id("busy"),
                cols: 100.try_into().unwrap(),
                rows: 30.try_into().unwrap(),
            }),
        ),
        (
            Op::Write,
            Call::Write(WriteParams {
                session_id: id("busy"),
                data: Base64(b"typed".to_vec()),
            }),
        ),
        (Op::Kill, close("busy")),
    ];
    let mut actor = fixture.greeted().await;
    for (op, call) in calls {
        fixture.gates.hold(op);
        let asked = actor.request(call).await;
        fixture.arrival(op).await;

        // A session being closed is out of the list before its terminal is gone.
        let listed: &[&str] = if op == Op::Kill {
            &["live"]
        } else {
            &["busy", "live"]
        };
        let served_in = others_are_served(&fixture, &mut watcher, listed).await;
        fixture.assert_still_held(op);
        println!("{op:?} held: hello, list and live output served in {served_in:?}");

        fixture.gates.release(op);
        let response = actor.response().await;
        assert_eq!(response.id, Some(asked));
        response.outcome.unwrap();
    }

    let busy = &fixture.fake.terminals()[1];
    assert_eq!(
        busy.size(),
        TerminalSize {
            rows: 30,
            cols: 100
        }
    );
    assert_eq!(busy.input(), b"typed");
    assert!(busy.was_killed());
    assert_eq!(names(&mut actor).await, ["live"]);
}

#[tokio::test]
async fn a_session_being_opened_keeps_its_name_and_its_slot() {
    let mut fixture = fixture("max_sessions = 1");
    fixture.gates.hold(Op::Spawn);
    let host = Arc::clone(&fixture.host);
    let opening = tokio::task::spawn_blocking(move || host.open(&id("first"), &spec()));
    fixture.arrival(Op::Spawn).await;

    let host = &fixture.host;
    let started = Instant::now();
    assert!(host.sessions().is_empty());
    assert!(host.session_ids().is_empty());
    let listed_in = started.elapsed();
    // A second open of the same name loses, and the reserved slot counts against the limit.
    assert_eq!(
        host.open(&id("first"), &spec()).err(),
        Some(PtyError::SessionExists(id("first")))
    );
    assert_eq!(
        host.open(&id("second"), &spec()).err(),
        Some(PtyError::TooManySessions { limit: 1 })
    );
    let unknown = Some(PtyError::UnknownSession(id("first")));
    assert_eq!(
        host.subscribe(&id("first"), ReplayFrom::Oldest).err(),
        unknown
    );
    assert_eq!(host.write(&id("first"), b"x").err(), unknown);
    assert_eq!(host.close(&id("first")).await.err(), unknown);
    fixture.assert_still_held(Op::Spawn);
    println!("spawn held: the session list was read in {listed_in:?}");

    fixture.gates.release(Op::Spawn);
    opening.await.unwrap().unwrap();
    assert_eq!(host.session_ids(), [id("first")]);
    assert_eq!(fixture.fake.terminals().len(), 1);
}

#[tokio::test]
async fn a_spawn_that_fails_or_panics_gives_the_name_and_the_slot_back() {
    let fixture = fixture("max_sessions = 1");
    let host = &fixture.host;

    assert!(matches!(
        host.open(&id("only"), &SpawnSpec::default()),
        Err(PtyError::Spawn(_))
    ));
    let panicking = SpawnSpec {
        program: PANIC_PROGRAM.to_owned(),
        ..SpawnSpec::default()
    };
    assert!(catch_unwind(AssertUnwindSafe(|| host.open(&id("only"), &panicking))).is_err());
    assert!(host.session_ids().is_empty());

    host.open(&id("only"), &spec()).unwrap();
    assert_eq!(host.session_ids(), [id("only")]);
}
