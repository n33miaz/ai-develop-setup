//! Session behavior on the fake terminal: no real process, no sleeping.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ads_ptyhost::{
    ExitStatus, FakeBackend, HostConfig, OutputChunk, PtyError, PtyHost, ReplayFrom, Seq,
    SessionId, SpawnSpec, Subscription, TerminalSize,
};

const STEP_TIMEOUT: Duration = Duration::from_secs(10);

fn spec() -> SpawnSpec {
    SpawnSpec {
        program: "fake-shell".to_owned(),
        ..SpawnSpec::default()
    }
}

/// Opens a session under a name no other session of this test binary has.
fn open(host: &PtyHost, spec: &SpawnSpec) -> SessionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = SessionId::new(format!("s{:03}", NEXT.fetch_add(1, Ordering::Relaxed))).unwrap();
    host.open(&id, spec).unwrap();
    id
}

fn host_with(backend: &Arc<FakeBackend>, overrides: &str) -> PtyHost {
    let config = HostConfig::from_toml_str(overrides).unwrap();
    PtyHost::new(Arc::clone(backend) as _, config).unwrap()
}

async fn next(subscription: &mut Subscription) -> Option<OutputChunk> {
    tokio::time::timeout(STEP_TIMEOUT, subscription.recv())
        .await
        .expect("no output within the step timeout")
}

async fn take(subscription: &mut Subscription, count: usize) -> Vec<OutputChunk> {
    let mut chunks = Vec::new();
    for _ in 0..count {
        chunks.push(next(subscription).await.expect("output ended early"));
    }
    chunks
}

async fn drain(subscription: &mut Subscription) -> Vec<OutputChunk> {
    let mut chunks = Vec::new();
    while let Some(chunk) = next(subscription).await {
        chunks.push(chunk);
    }
    chunks
}

fn seqs(chunks: &[OutputChunk]) -> Vec<u64> {
    chunks.iter().map(|c| c.seq.0).collect()
}

fn text(chunks: &[OutputChunk]) -> String {
    let bytes: Vec<u8> = chunks
        .iter()
        .flat_map(|c| c.bytes.iter().copied())
        .collect();
    String::from_utf8(bytes).unwrap()
}

/// Waits until the session has numbered `count` chunks.
async fn wait_until_numbered(host: &PtyHost, id: &SessionId, count: u64) {
    tokio::time::timeout(STEP_TIMEOUT, async {
        while host.subscribe(id, ReplayFrom::Seq(Seq(count + 1))).is_err() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the session never reached the expected seq");
}

#[tokio::test]
async fn live_output_arrives_numbered_and_in_order() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());
    let mut subscription = host.subscribe(&id, ReplayFrom::Oldest).unwrap();

    let terminal = &backend.terminals()[0];
    terminal.emit(b"one ");
    terminal.emit(b"two ");
    terminal.emit(b"three");

    let chunks = take(&mut subscription, 3).await;
    assert_eq!(seqs(&chunks), [1, 2, 3]);
    assert_eq!(text(&chunks), "one two three");
    host.close(&id).await.unwrap();
}

#[tokio::test]
async fn reconnect_replays_from_the_requested_seq_then_goes_live() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());
    let terminal = &backend.terminals()[0];

    let mut first = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    terminal.emit(b"a");
    terminal.emit(b"b");
    let seen = take(&mut first, 2).await;
    let last_seen = seen.last().unwrap().seq.0;
    drop(first);

    // Output keeps coming while the client is away.
    terminal.emit(b"c");
    terminal.emit(b"d");
    wait_until_numbered(&host, &id, 4).await;

    let mut second = host
        .subscribe(&id, ReplayFrom::Seq(Seq(last_seen + 1)))
        .unwrap();
    terminal.emit(b"e");
    let resumed = take(&mut second, 3).await;
    assert_eq!(seqs(&resumed), [3, 4, 5]);
    assert_eq!(text(&resumed), "cde");
    host.close(&id).await.unwrap();
}

#[tokio::test]
async fn many_reconnects_see_every_byte_once() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());
    let terminal = backend.terminals()[0].clone();

    let expected: String = (0..200).map(|n| format!("line {n}\n")).collect();
    let writer = std::thread::spawn(move || {
        for n in 0..200 {
            terminal.emit(format!("line {n}\n").as_bytes());
        }
        terminal.exit(0);
    });

    // The client drops the connection after every few chunks and resumes from where it stopped.
    let mut collected = Vec::new();
    let mut next_seq = 1;
    loop {
        let mut subscription = host.subscribe(&id, ReplayFrom::Seq(Seq(next_seq))).unwrap();
        let mut ended = false;
        for _ in 0..7 {
            match next(&mut subscription).await {
                Some(chunk) => {
                    assert_eq!(chunk.seq.0, next_seq, "gap or repeat in the stream");
                    next_seq += 1;
                    collected.push(chunk);
                }
                None => {
                    ended = true;
                    break;
                }
            }
        }
        if ended {
            break;
        }
    }
    writer.join().unwrap();

    assert_eq!(text(&collected), expected);
    assert_eq!(host.close(&id).await.unwrap(), ExitStatus { code: 0 });
}

#[tokio::test]
async fn replay_is_bounded_and_refuses_what_it_dropped() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(
        &backend,
        "replay_capacity_bytes = 16\nread_buffer_bytes = 8",
    );
    let id = open(&host, &spec());
    let terminal = &backend.terminals()[0];

    for n in 0..10 {
        terminal.emit(format!("chunk{n:03}").as_bytes());
    }
    wait_until_numbered(&host, &id, 10).await;

    let mut replay = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    let held = take(&mut replay, 2).await;
    assert_eq!(seqs(&held), [9, 10]);
    assert_eq!(text(&held), "chunk008chunk009");

    assert_eq!(
        host.subscribe(&id, ReplayFrom::Seq(Seq(3))).err(),
        Some(PtyError::ReplayEvicted {
            requested: Seq(3),
            oldest: Seq(9)
        })
    );
    assert_eq!(
        host.subscribe(&id, ReplayFrom::Seq(Seq(50))).err(),
        Some(PtyError::ReplayAhead {
            requested: Seq(50),
            next: Seq(11)
        })
    );
    host.close(&id).await.unwrap();
}

#[tokio::test]
async fn slow_subscriber_is_dropped_and_recovers_by_replay() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "subscriber_queue_chunks = 2");
    let id = open(&host, &spec());
    let terminal = &backend.terminals()[0];

    let mut slow = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    for n in 0..6 {
        terminal.emit(format!("{n}").as_bytes());
    }
    wait_until_numbered(&host, &id, 6).await;

    // The queue held two chunks; the third overflowed it and ended the subscription.
    let before = drain(&mut slow).await;
    assert_eq!(seqs(&before), [1, 2]);

    let mut resumed = host.subscribe(&id, ReplayFrom::Seq(Seq(3))).unwrap();
    let after = take(&mut resumed, 4).await;
    assert_eq!(seqs(&after), [3, 4, 5, 6]);
    assert_eq!(format!("{}{}", text(&before), text(&after)), "012345");
    host.close(&id).await.unwrap();
}

#[tokio::test]
async fn input_and_resize_reach_the_terminal() {
    let backend = Arc::new(FakeBackend::with_echo());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());
    let mut subscription = host.subscribe(&id, ReplayFrom::Oldest).unwrap();

    host.write(&id, b"dir\r").unwrap();
    host.resize(
        &id,
        TerminalSize {
            rows: 40,
            cols: 120,
        },
    )
    .unwrap();

    let terminal = &backend.terminals()[0];
    assert_eq!(terminal.input(), b"dir\r");
    assert_eq!(
        terminal.size(),
        TerminalSize {
            rows: 40,
            cols: 120
        }
    );
    assert_eq!(text(&take(&mut subscription, 1).await), "dir\r");
    host.close(&id).await.unwrap();
}

#[tokio::test]
async fn natural_exit_ends_the_stream_and_keeps_the_replay() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());
    let mut subscription = host.subscribe(&id, ReplayFrom::Oldest).unwrap();

    let terminal = &backend.terminals()[0];
    terminal.emit(b"bye");
    terminal.exit(7);

    assert_eq!(text(&drain(&mut subscription).await), "bye");
    assert_eq!(host.wait_exit(&id).await.unwrap(), ExitStatus { code: 7 });
    assert!(!terminal.was_killed());

    let mut late = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    assert_eq!(text(&drain(&mut late).await), "bye");
    assert_eq!(host.close(&id).await.unwrap(), ExitStatus { code: 7 });
}

#[tokio::test]
async fn close_kills_the_terminal_and_forgets_the_session() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());
    let mut subscription = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    assert_eq!(host.exit_status(&id).unwrap(), None);

    host.close(&id).await.unwrap();

    assert!(backend.terminals()[0].was_killed());
    assert!(next(&mut subscription).await.is_none());
    assert!(host.session_ids().is_empty());
    assert_eq!(
        host.write(&id, b"x").err(),
        Some(PtyError::UnknownSession(id.clone()))
    );
    assert_eq!(
        host.close(&id).await.err(),
        Some(PtyError::UnknownSession(id.clone()))
    );
}

#[tokio::test]
async fn dropping_the_host_kills_open_terminals() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    open(&host, &spec());
    drop(host);
    assert!(backend.terminals()[0].was_killed());
}

#[tokio::test]
async fn session_limit_is_enforced_and_failed_spawns_take_no_slot() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "max_sessions = 2");

    let name = |text: &str| SessionId::new(text).unwrap();
    assert!(matches!(
        host.open(&name("empty"), &SpawnSpec::default()),
        Err(PtyError::Spawn(_))
    ));
    let first = open(&host, &spec());
    let second = open(&host, &spec());
    assert_eq!(
        host.open(&name("over"), &spec()).err(),
        Some(PtyError::TooManySessions { limit: 2 })
    );
    // A name in use is refused, and says so even when the host is full.
    assert_eq!(
        host.open(&first, &spec()).err(),
        Some(PtyError::SessionExists(first.clone()))
    );
    assert_eq!(host.session_ids(), [first.clone(), second.clone()]);

    host.close(&first).await.unwrap();
    let third = open(&host, &spec());
    assert_ne!(third, first);
    host.close(&second).await.unwrap();
    host.close(&third).await.unwrap();
}

#[tokio::test]
async fn the_child_environment_is_exactly_what_the_spec_carries() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let wanted = SpawnSpec {
        env: vec![("LANG".to_owned(), "C".to_owned())],
        ..spec()
    };
    let id = open(&host, &wanted);
    assert_eq!(backend.terminals()[0].spec(), wanted);
    host.close(&id).await.unwrap();
}

#[tokio::test]
async fn dropped_subscriptions_leave_nothing_behind_on_an_idle_session() {
    const CYCLES: usize = 1000;
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "");
    let id = open(&host, &spec());

    for _ in 0..CYCLES {
        drop(host.subscribe(&id, ReplayFrom::Oldest).unwrap());
    }
    assert_eq!(host.subscriber_count(&id).unwrap(), 0);

    // Held together and dropped out of order: each one takes only itself away.
    let mut held: Vec<_> = (0..CYCLES)
        .map(|_| host.subscribe(&id, ReplayFrom::Oldest).unwrap())
        .collect();
    assert_eq!(host.subscriber_count(&id).unwrap(), CYCLES);
    held.retain({
        let mut index = 0;
        move |_| {
            index += 1;
            index % 2 == 0
        }
    });
    assert_eq!(host.subscriber_count(&id).unwrap(), CYCLES / 2);
    drop(held);
    assert_eq!(host.subscriber_count(&id).unwrap(), 0);

    // Not one byte of output was needed for any of it.
    assert_eq!(host.sessions()[0].next_seq, Seq(1));
}

#[tokio::test]
async fn a_lagging_subscriber_is_forgotten_as_soon_as_it_lags() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "subscriber_queue_chunks = 2");
    let id = open(&host, &spec());
    let mut lagging = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    let mut reading = host.subscribe(&id, ReplayFrom::Oldest).unwrap();

    let terminal = &backend.terminals()[0];
    for n in 1..=3_u8 {
        terminal.emit(&[n]);
        assert_eq!(next(&mut reading).await.unwrap().seq, Seq(u64::from(n)));
    }
    // The third chunk found the queue of the one that never read full.
    assert_eq!(host.subscriber_count(&id).unwrap(), 1);
    assert_eq!(seqs(&drain(&mut lagging).await), [1, 2]);
    drop(lagging);
    assert_eq!(host.subscriber_count(&id).unwrap(), 1);
    drop(reading);
    assert_eq!(host.subscriber_count(&id).unwrap(), 0);
}
