//! Acceptance of the separate host process: the client (the core) dies and reconnects through
//! the real local channel again and again, and not one byte of terminal output is lost or
//! repeated. Fake terminal: the emitted stream is known byte by byte.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use ads_ptyhost::channel::Listener;
use ads_ptyhost::wire::{
    Call, NoParams, Notification, OpenParams, ResultBody, SessionId, SessionParams,
};
use ads_ptyhost::{FakeBackend, FakeTerminal, HostConfig, PtyHost, server};
use common::client::{Client, Incoming, Transcript, open_channel, unique_endpoint};

const DROPS: u64 = 20;

/// Deterministic bytes covering every value, so nothing in the path can get away with
/// treating output as text.
struct Stream {
    state: u32,
    emitted: Vec<u8>,
}

impl Stream {
    fn new() -> Self {
        Self {
            state: 0x2545_f491,
            emitted: Vec::new(),
        }
    }

    fn next(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.state >> 8
    }

    /// Emits `count` reads of 1 to 3000 bytes and returns how many bytes that was.
    fn emit(&mut self, terminal: &FakeTerminal, count: usize) -> u64 {
        let mut total = 0;
        for _ in 0..count {
            let len = 1 + self.next() as usize % 3000;
            let chunk: Vec<u8> = (0..len).map(|_| self.next() as u8).collect();
            terminal.emit(&chunk);
            total += len as u64;
            self.emitted.extend(chunk);
        }
        total
    }
}

fn start(backend: &Arc<FakeBackend>, label: &str) -> (String, Arc<PtyHost>) {
    let endpoint = unique_endpoint(label);
    let host = Arc::new(PtyHost::new(Arc::clone(backend) as _, HostConfig::defaults()).unwrap());
    let listener = Listener::bind(&endpoint).unwrap();
    tokio::spawn(server::serve(Arc::clone(&host), listener));
    (endpoint, host)
}

fn open(session_id: &SessionId) -> Call {
    Call::Open(OpenParams {
        session_id: session_id.clone(),
        program: "fake-shell".to_owned(),
        args: Vec::new(),
        cwd: None,
        env: BTreeMap::new(),
        cols: 80.try_into().unwrap(),
        rows: 24.try_into().unwrap(),
    })
}

fn accept(transcript: &mut Transcript, session_id: &SessionId, incoming: Incoming) {
    match incoming {
        Incoming::Output { header, bytes } => transcript.accept(session_id, &header, &bytes),
        Incoming::Notification(other) => panic!("unexpected notification: {other:?}"),
    }
}

#[tokio::test]
async fn client_dropped_twenty_times_loses_and_repeats_nothing() {
    let started = Instant::now();
    let backend = Arc::new(FakeBackend::new());
    let (endpoint, _host) = start(&backend, "drops");
    let session_id = SessionId::new("sess-accept").unwrap();

    let mut first = Client::connect(&endpoint).await;
    first.call(open(&session_id)).await.unwrap();
    drop(first);
    let terminal = backend.terminals()[0].clone();

    let mut stream = Stream::new();
    let mut transcript = Transcript::default();
    let mut cut_inside_a_frame = 0;
    for drop_number in 0..DROPS {
        // Output while nobody is connected: it can only come back through the replay ring.
        let mut pending = stream.emit(&terminal, 3);

        let mut client = Client::connect(&endpoint).await;
        let bounds = client
            .subscribe(&session_id, Some(transcript.last_seq + 1))
            .await
            .unwrap();
        assert_eq!(
            bounds.oldest_seq.get(),
            1,
            "the ring must hold the whole test"
        );

        // Output while connected: it arrives live, right behind the replay.
        pending += stream.emit(&terminal, 3);

        if drop_number % 2 == 0 {
            // Dies between two frames, with output still on its way.
            for _ in 0..2 + drop_number % 3 {
                let incoming = client.next().await.expect("channel closed");
                accept(&mut transcript, &session_id, incoming);
            }
        } else {
            // Dies in the middle of a frame: whatever was cut is not counted as seen.
            let (whole_frames, inside_a_frame) = client.read_cut_at(pending * 2 / 3).await;
            for incoming in whole_frames {
                accept(&mut transcript, &session_id, incoming);
            }
            cut_inside_a_frame += u64::from(inside_a_frame);
        }
        drop(client);
    }
    assert_eq!(
        cut_inside_a_frame,
        DROPS / 2,
        "a cut fell on a frame boundary"
    );

    // Last connection: the rest of the replay, the tail of the stream and the exit.
    let mut client = Client::connect(&endpoint).await;
    client
        .subscribe(&session_id, Some(transcript.last_seq + 1))
        .await
        .unwrap();
    stream.emit(&terminal, 5);
    terminal.exit(0);
    let exited = loop {
        match client.next().await.expect("channel closed") {
            Incoming::Notification(Notification::SessionExited(exited)) => break exited,
            incoming => accept(&mut transcript, &session_id, incoming),
        }
    };

    assert_eq!(exited.last_seq, transcript.last_seq);
    assert_eq!(exited.exit_code, Some(0));
    assert_eq!(transcript.last_seq, DROPS * 6 + 5, "one seq per read");
    assert_eq!(transcript.bytes.len(), stream.emitted.len());
    assert!(
        transcript.bytes == stream.emitted,
        "received bytes differ from the emitted ones"
    );
    println!(
        "{DROPS} drops: {} bytes in {} chunks, identical, in {:?}",
        transcript.bytes.len(),
        transcript.last_seq,
        started.elapsed()
    );
}

#[tokio::test]
async fn clients_that_leave_without_a_word_do_not_take_the_host_down() {
    let backend = Arc::new(FakeBackend::new());
    let (endpoint, _host) = start(&backend, "rude");
    let session_id = SessionId::new("sess-kept").unwrap();
    let mut keeper = Client::connect(&endpoint).await;
    keeper.call(open(&session_id)).await.unwrap();

    for _ in 0..DROPS {
        drop(open_channel(&endpoint).await);
    }

    let mut client = Client::connect(&endpoint).await;
    let ResultBody::SessionList(list) = client.call(Call::List(NoParams {})).await.unwrap() else {
        panic!("unexpected list result");
    };
    assert_eq!(list.sessions.len(), 1);
    assert_eq!(list.sessions[0].session_id, session_id);
    let close = Call::Close(SessionParams { session_id });
    assert!(keeper.call(close).await.is_ok());
}

#[tokio::test]
async fn a_second_host_cannot_take_a_channel_in_use() {
    let backend = Arc::new(FakeBackend::new());
    let (endpoint, _host) = start(&backend, "taken");
    assert!(Listener::bind(&endpoint).is_err());
    // And the first one is still there.
    Client::connect(&endpoint).await;
}

#[cfg(unix)]
#[tokio::test]
async fn socket_is_for_its_owner_only_and_goes_away_with_the_listener() {
    use std::os::unix::fs::PermissionsExt;

    let endpoint = unique_endpoint("mode");
    let listener = Listener::bind(&endpoint).unwrap();
    let mode = std::fs::metadata(&endpoint).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    drop(listener);
    assert!(!std::path::Path::new(&endpoint).exists());
    // What a crashed host leaves behind does not block the next one.
    drop(std::os::unix::net::UnixListener::bind(&endpoint).unwrap());
    assert!(std::path::Path::new(&endpoint).exists());
    drop(Listener::bind(&endpoint).unwrap());
}
