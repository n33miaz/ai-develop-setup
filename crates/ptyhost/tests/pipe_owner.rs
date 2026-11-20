//! The check of who serves the pipe, through the public entries, failing over and over on real
//! queries: it refuses every time, names the query that failed and releases every handle it
//! opened. The handle count is per process, so this binary holds the one test that counts; the
//! others are manual and ignored.
#![cfg(windows)]

use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

use ads_ptyhost::channel::{ConnectError, connect};
use ads_ptyhost::win::{
    LocalUser, PipeQuery, PipeQueryFailed, check_pipe_server, create_user_pipe, current_user_sid,
    is_own_server, local_user, own_handle_count, pipe_server,
};
use tokio::io::AsyncReadExt;

const WARMUP_CYCLES: usize = 3;
const CYCLES: usize = 40;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const STABLE_SAMPLES: usize = 3;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Access that opens the pipe and finds its server, but cannot read who owns it.
const FILE_READ_ATTRIBUTES: u32 = 0x80;

fn endpoint(label: &str, cycle: usize) -> String {
    format!(
        r"\\.\pipe\ads-ptyhost-owner-{}-{label}-{cycle}",
        std::process::id()
    )
}

fn file_endpoint(cycle: usize) -> String {
    let name = format!("pipe-owner-file-{}-{cycle}", std::process::id());
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    path.to_str().unwrap().to_owned()
}

fn stranger() -> LocalUser {
    LocalUser {
        user: "S-1-5-21-1000-2000-3000-1002".to_owned(),
        default_owner: "S-1-5-21-1000-2000-3000-1002".to_owned(),
    }
}

/// One pass through each way the check fails.
async fn cycle(cycle: usize, not_a_pipe: &std::fs::File) {
    // The first query fails: the handle given is not a pipe.
    let error = check_pipe_server(not_a_pipe.as_raw_handle()).unwrap_err();
    assert_eq!(
        PipeQueryFailed::of(&error),
        Some(PipeQuery::ServerProcess),
        "{error}"
    );

    // The last query fails: a client that may read the pipe's attributes but not its owner
    // opens the server process and its token, then cannot read who owns the pipe.
    let owner_endpoint = endpoint("owner", cycle);
    let _server = create_user_pipe(&owner_endpoint, &current_user_sid().unwrap(), true).unwrap();
    let limited = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .open(&owner_endpoint)
        .unwrap();
    let error = check_pipe_server(limited.as_raw_handle()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
    assert_eq!(
        PipeQueryFailed::of(&error),
        Some(PipeQuery::ObjectOwner),
        "{error}"
    );

    // Every query succeeds and reads this user; the decision refuses another identity.
    let whole_endpoint = endpoint("whole", cycle);
    let _whole = create_user_pipe(&whole_endpoint, &current_user_sid().unwrap(), true).unwrap();
    let client = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&whole_endpoint)
        .unwrap();
    let local = local_user().unwrap();
    let read = pipe_server(client.as_raw_handle()).unwrap();
    assert_eq!(read.process_user, local.user);
    assert!(
        read.object_owner == local.user || read.object_owner == local.default_owner,
        "{read:?} {local:?}"
    );
    assert!(!is_own_server(&stranger(), &read));

    // The public entry, refused by a regular file at the endpoint (it opens like a pipe and
    // nobody can be shown to serve it): the stream it opened is dropped.
    let file = file_endpoint(cycle);
    std::fs::write(&file, b"").unwrap();
    let result = connect(&file).await;
    assert!(
        matches!(result, Err(ConnectError::NotOwnedByUser)),
        "{result:?}"
    );
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
    for index in 0..WARMUP_CYCLES {
        cycle(index, &not_a_pipe).await;
    }
    let before = settle(u32::MAX).await;
    for index in WARMUP_CYCLES..WARMUP_CYCLES + CYCLES {
        cycle(index, &not_a_pipe).await;
    }
    let after = settle(before).await;
    // Nothing reached the refused endpoints; each file goes once its handle is closed.
    for index in 0..WARMUP_CYCLES + CYCLES {
        let file = file_endpoint(index);
        assert!(std::fs::read(&file).unwrap().is_empty(), "{file}");
        std::fs::remove_file(&file).unwrap();
    }
    println!("pipe owner refusals: {CYCLES} cycles; handles before: {before}, after: {after}");
}

// Manual checks: they need a second account or another elevation level. How to run them and
// the expected results are in the README, "Testes manuais do dono do pipe".

/// What opening a pipe whose access list leaves the user out returns.
const ERROR_ACCESS_DENIED: i32 = 5;
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
#[ignore = "manual: run by a second account, see the ptyhost README"]
async fn serve_for_another_account() {
    let instructions =
        "Run it from a shell of another account; see the ptyhost README, manual pipe owner tests.";
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
/// `ADS_PIPE_OWNER_EXPECT` (`accepted`, `refused` or `unavailable`, from the table in the README).
#[tokio::test]
#[ignore = "manual: needs a server of another account or elevation level, see the ptyhost README"]
async fn connect_to_a_prepared_server() {
    let instructions = "Start one of the servers of the table in the ptyhost README first.";
    let endpoint = required(ENDPOINT_VARIABLE, instructions);
    let expect = required(EXPECT_VARIABLE, instructions);
    let result = connect(&endpoint).await;
    match expect.as_str() {
        "accepted" => assert!(result.is_ok(), "{:?}", result.err()),
        "refused" => assert!(
            matches!(result, Err(ConnectError::NotOwnedByUser)),
            "{:?}",
            result.err()
        ),
        // The access list of the pipe keeps the client out before any identity is read. Only
        // a pipe that exists answers "access denied": a wrong endpoint answers "not found" and
        // fails here, and a busy one ("all instances busy") too.
        "unavailable" => match result {
            Err(ConnectError::Unavailable(error)) => assert_eq!(
                error.raw_os_error(),
                Some(ERROR_ACCESS_DENIED),
                "not refused by the access list: {error}"
            ),
            other => panic!("expected access denied, got {:?}", other.err()),
        },
        other => panic!(
            "{EXPECT_VARIABLE} must be `accepted`, `refused` or `unavailable`, not `{other}`"
        ),
    }
}
