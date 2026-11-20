//! The check of who serves the pipe, failing over and over on real queries, refuses every time
//! and releases every handle it opened. The handle count is per process, so this binary holds
//! the one test that counts; the others are manual and ignored (see each one).
#![cfg(windows)]

use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::time::{Duration, Instant};

use ads_ptyhost::channel::{ConnectError, connect, connect_checked};
use ads_ptyhost::win::{
    LocalUser, check_pipe_server, check_pipe_server_as, create_user_pipe, current_user_sid,
    own_handle_count, pipe_server,
};
use tokio::io::AsyncReadExt;

const WARMUP_ROUNDS: usize = 3;
const ROUNDS: usize = 40;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const STABLE_SAMPLES: usize = 3;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Access that opens the pipe and finds its server, but cannot read who owns it.
const FILE_READ_ATTRIBUTES: u32 = 0x80;

fn endpoint(label: &str, round: usize) -> String {
    format!(
        r"\\.\pipe\ads-ptyhost-owner-{}-{label}-{round}",
        std::process::id()
    )
}

fn stranger() -> LocalUser {
    LocalUser {
        user: "S-1-5-21-1000-2000-3000-1002".to_owned(),
        default_owner: "S-1-5-21-1000-2000-3000-1002".to_owned(),
    }
}

/// A client refused by `check` on a fresh pipe: the server saw it connect and received nothing.
async fn refused_on_a_real_pipe(
    label: &str,
    round: usize,
    check: impl FnOnce(RawHandle) -> io::Result<()>,
) {
    let endpoint = endpoint(label, round);
    let mut server = create_user_pipe(&endpoint, &current_user_sid().unwrap(), true).unwrap();
    {
        let mut waiting = std::pin::pin!(server.connect());
        // Waiting before the client opens, so a client already gone cannot fail the wait.
        tokio::select! {
            biased;
            early = &mut waiting => panic!("connected with no client: {early:?}"),
            () = std::future::ready(()) => {}
        }
        let result = connect_checked(&endpoint, check);
        assert!(
            matches!(result, Err(ConnectError::NotOwnedByUser)),
            "{label}: {result:?}"
        );
        tokio::time::timeout(IO_TIMEOUT, waiting)
            .await
            .expect("the server never saw the client")
            .unwrap();
    }
    let mut received = Vec::new();
    tokio::time::timeout(IO_TIMEOUT, server.read_to_end(&mut received))
        .await
        .expect("the refused client left the pipe open")
        .unwrap();
    assert!(received.is_empty(), "{label}: {} bytes", received.len());
}

/// One round of each way the queries fail.
async fn round(round: usize, not_a_pipe: &std::fs::File) {
    // The first query fails: the handle given is not a pipe.
    refused_on_a_real_pipe("first", round, |_| {
        check_pipe_server(not_a_pipe.as_raw_handle())
    })
    .await;

    // Every query succeeds and the identity does not match: the server process, its token and
    // the pipe's descriptor were all opened before the refusal.
    let stranger = stranger();
    refused_on_a_real_pipe("whole", round, |pipe| check_pipe_server_as(&stranger, pipe)).await;

    // The last query fails: a client that may read the pipe's attributes but not its owner
    // finds the server process and its token, then cannot read who owns the pipe.
    let endpoint = endpoint("owner", round);
    let _server = create_user_pipe(&endpoint, &current_user_sid().unwrap(), true).unwrap();
    let limited = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .open(&endpoint)
        .unwrap();
    let error = pipe_server(limited.as_raw_handle()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert!(check_pipe_server(limited.as_raw_handle()).is_err());
}

/// Waits for the handle count to stop moving at or below `limit` and returns it.
async fn settle(limit: u32) -> u32 {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let mut previous = None;
    let mut stable = 0;
    loop {
        let count = own_handle_count().unwrap();
        if count <= limit {
            stable = if previous == Some(count) {
                stable + 1
            } else {
                1
            };
            previous = Some(count);
        } else {
            stable = 0;
            previous = None;
        }
        if stable == STABLE_SAMPLES {
            return count;
        }
        assert!(
            Instant::now() < deadline,
            "handle count grew from {limit} to {count}"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[tokio::test]
async fn repeated_query_failures_refuse_and_release_every_handle() {
    let not_a_pipe = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
    for index in 0..WARMUP_ROUNDS {
        round(index, &not_a_pipe).await;
    }
    let before = settle(u32::MAX).await;
    for index in WARMUP_ROUNDS..WARMUP_ROUNDS + ROUNDS {
        round(index, &not_a_pipe).await;
    }
    let after = settle(before).await;
    println!("pipe owner refusals: {ROUNDS} rounds; handles before: {before}, after: {after}");
}

// Manual checks. They need a second account or a different elevation level, so they never run
// on their own: each one panics with its instructions when the variables it reads are missing.
//
// Build the binary once (`cargo test -p ads-ptyhost --test pipe_owner --no-run` prints its
// path) and copy it to a folder the other account can read and run, e.g. under `C:\Users\Public`.
// The client side runs with `cargo test -p ads-ptyhost --test pipe_owner -- --ignored --exact
// <name>`; the side of the other account runs the copied binary with `--ignored --exact <name>`
// from a shell opened with `runas /user:<other account> cmd`.
//
// Expected results, client side:
// | Server                                                     | Client           | Result   |
// |------------------------------------------------------------|------------------|----------|
// | `serve_for_another_account`, run by another account        | any              | refused  |
// | host of the same user, both non-elevated                   | non-elevated     | accepted |
// | host of the same user, both elevated                       | elevated         | accepted |
// | host of the same user, elevated (owner: administrators)    | non-elevated     | refused  |
// | host of the same user, non-elevated                        | elevated         | accepted |
// | host of another administrator, elevated                    | elevated         | refused  |
//
// "Host of the same user" is `ads-ptyhost` started with `channel_path` set to the endpoint; the
// owner of its pipe is whatever the token of that process gives its objects.

const ENDPOINT_VARIABLE: &str = "ADS_PIPE_OWNER_ENDPOINT";
const CLIENT_VARIABLE: &str = "ADS_PIPE_OWNER_CLIENT_SID";
const EXPECT_VARIABLE: &str = "ADS_PIPE_OWNER_EXPECT";

fn required(name: &str, instructions: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => value,
        _ => panic!("{name} is not set. {instructions}"),
    }
}

/// Server side, run by ANOTHER account: a pipe whose access list lets the client user in.
/// Variables: `ADS_PIPE_OWNER_ENDPOINT` (e.g. `\\.\pipe\ads-owner-check`) and
/// `ADS_PIPE_OWNER_CLIENT_SID` (the client user's identifier, from `whoami /user`). Waits up to
/// two minutes for one client and fails if it receives any byte.
#[tokio::test]
#[ignore = "manual: run by a second account, see the comment above"]
async fn serve_for_another_account() {
    let instructions =
        "Run from a shell of another account; see the comment in tests/pipe_owner.rs.";
    let endpoint = required(ENDPOINT_VARIABLE, instructions);
    let client = required(CLIENT_VARIABLE, instructions);
    assert_ne!(client, current_user_sid().unwrap(), "{instructions}");
    let mut server = create_user_pipe(&endpoint, &client, true).unwrap();
    tokio::time::timeout(Duration::from_secs(120), server.connect())
        .await
        .expect("no client within two minutes")
        .unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(IO_TIMEOUT, server.read_to_end(&mut received))
        .await
        .expect("the client left the pipe open")
        .unwrap();
    println!("bytes received: {}", received.len());
    assert!(
        received.is_empty(),
        "{} bytes reached the server",
        received.len()
    );
}

/// Client side: connects to `ADS_PIPE_OWNER_ENDPOINT` with the real check and compares with
/// `ADS_PIPE_OWNER_EXPECT` (`accepted` or `refused`, from the table above).
#[tokio::test]
#[ignore = "manual: needs a server of another account or elevation level, see the comment above"]
async fn connect_to_a_prepared_server() {
    let instructions = "Start one of the servers in the table in tests/pipe_owner.rs first.";
    let endpoint = required(ENDPOINT_VARIABLE, instructions);
    let expect = required(EXPECT_VARIABLE, instructions);
    let result = connect(&endpoint).await;
    // Printed for the record; a normal account cannot read another user's server.
    let opened = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .open(&endpoint);
    if let Ok(pipe) = opened {
        println!("server as read: {:?}", pipe_server(pipe.as_raw_handle()));
    }
    match expect.as_str() {
        "accepted" => assert!(result.is_ok(), "{:?}", result.err()),
        "refused" => assert!(
            matches!(result, Err(ConnectError::NotOwnedByUser)),
            "{:?}",
            result.err()
        ),
        other => panic!("{EXPECT_VARIABLE} must be `accepted` or `refused`, not `{other}`"),
    }
}
