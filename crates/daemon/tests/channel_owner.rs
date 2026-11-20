#![cfg(windows)]

use std::time::Duration;

use ads_daemon::pty_client::{ClientConfig, ClientError, PtyClient};
use ads_ptyhost::HostConfig;

/// Without a second account the test cannot create a pipe another user serves. A regular file
/// at the endpoint opens like one, and nobody can be shown to serve it: the client must refuse
/// it before sending `host.hello`.
#[tokio::test]
async fn endpoint_not_served_by_the_user_is_refused_before_hello() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("daemon-not-a-pipe-{}", std::process::id()));
    std::fs::write(&path, b"").unwrap();
    let config =
        HostConfig::from_toml_str(&format!("channel_path = '{}'\n", path.display())).unwrap();

    let result = PtyClient::connect(&config, ClientConfig::recommended(&config)).await;
    let error = result.err();
    // The runtime closes the handle once it has seen the end of its pending read.
    let mut written = std::fs::read(&path);
    for _ in 0..100 {
        if written.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        written = std::fs::read(&path);
    }
    let written = written.unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(error, Some(ClientError::NotOwnedByUser));
    assert!(
        written.is_empty(),
        "{} bytes reached the endpoint",
        written.len()
    );
}

const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Tells the child process started by `restricted_server` what to do: `<part>|<endpoint>`.
const RESTRICTED_VARIABLE: &str = "ADS_TEST_RESTRICTED_PIPE";

fn pipe_endpoint(label: &str) -> String {
    format!(r"\\.\pipe\ads-daemon-owner-{}-{label}", std::process::id())
}

fn config_for(endpoint: &str) -> HostConfig {
    HostConfig::from_toml_str(&format!("channel_path = '{endpoint}'\n")).unwrap()
}

const HELPER_TIMEOUT: Duration = Duration::from_secs(20);

/// Not a test: the child process of `restricted_server`. It creates the pipe and starts waiting
/// for a client, closes its own process or token to everyone but the system account, as a
/// server of another account would be, then says it is ready, serves one client and reports
/// how many bytes it received.
#[tokio::test]
#[ignore = "helper run in a child process by the restricted server tests"]
async fn restricted_server_helper() {
    use ads_ptyhost::win::{create_user_pipe, current_user_sid};
    use ads_testkit::{Restricted, restrict_own_process};
    use tokio::io::AsyncReadExt;

    let spec = std::env::var(RESTRICTED_VARIABLE)
        .expect("helper only: started by the restricted server tests");
    let (part, endpoint) = spec.split_once('|').unwrap();
    let part = match part {
        "process" => Restricted::Process,
        "token" => Restricted::Token,
        other => panic!("unknown part {other}"),
    };
    let mut server = create_user_pipe(endpoint, &current_user_sid().unwrap(), true).unwrap();
    {
        let mut waiting = std::pin::pin!(server.connect());
        // The wait starts before the client is told to come: a client already gone could not
        // fail it.
        tokio::select! {
            biased;
            early = &mut waiting => panic!("connected with no client: {early:?}"),
            () = std::future::ready(()) => {}
        }
        restrict_own_process(part).unwrap();
        println!("@@ready@@");
        tokio::time::timeout(HELPER_TIMEOUT, waiting)
            .await
            .expect("no client came")
            .unwrap();
    }
    let mut received = Vec::new();
    // A client that got through sends `host.hello` and waits; the test then drops it, and the
    // bytes are counted here all the same.
    tokio::time::timeout(HELPER_TIMEOUT, server.read_to_end(&mut received))
        .await
        .expect("the client left the pipe open")
        .unwrap();
    println!("@@received={}@@", received.len());
}

/// The public entry against a pipe served by a child process of this user whose process or
/// token nobody else may open: refused with `NotOwnedByUser`, and the server receives no byte,
/// so `host.hello` never left. The child is killed and reaped on every way out.
async fn restricted_server(part: &str) {
    let endpoint = pipe_endpoint(part);
    let mut helper = ads_testkit::HelperProcess::start(
        "restricted_server_helper",
        RESTRICTED_VARIABLE,
        &format!("{part}|{endpoint}"),
    )
    .unwrap();
    assert!(
        helper.wait_for("@@ready@@", HELPER_TIMEOUT).is_some(),
        "the helper never got ready"
    );

    let config = config_for(&endpoint);
    let client = tokio::spawn(async move {
        PtyClient::connect(&config, ClientConfig::recommended(&config))
            .await
            .err()
    });
    let refused = tokio::time::timeout(IO_TIMEOUT, client)
        .await
        .expect("the client got through and waits for an answer")
        .unwrap();
    assert_eq!(refused, Some(ClientError::NotOwnedByUser));

    // Off the runtime thread: the runtime has to keep running to close the refused stream.
    let (helper, received) = tokio::task::spawn_blocking(move || {
        let received = helper.wait_for("@@received=", HELPER_TIMEOUT);
        (helper, received)
    })
    .await
    .unwrap();
    assert_eq!(received.as_deref(), Some("0"), "bytes reached the pipe");
    let status = helper
        .finish(HELPER_TIMEOUT)
        .expect("the helper did not end");
    assert!(status.success(), "{status}");
}

#[tokio::test]
async fn a_server_whose_process_cannot_be_opened_receives_no_hello() {
    restricted_server("process").await;
}

#[tokio::test]
async fn a_server_whose_token_cannot_be_opened_receives_no_hello() {
    restricted_server("token").await;
}

/// Control for the tests above: the same public entry on a pipe of this user sends a whole
/// `host.hello` as its first frame.
#[tokio::test]
async fn own_pipe_receives_a_whole_hello_first() {
    use ads_ptyhost::win::{create_user_pipe, current_user_sid};
    use ads_ptyhost::wire::{self, Call, Frame, Request};

    let endpoint = pipe_endpoint("own");
    let config = config_for(&endpoint);
    let mut server = create_user_pipe(&endpoint, &current_user_sid().unwrap(), true).unwrap();
    let client = tokio::spawn({
        let config = config.clone();
        async move {
            PtyClient::connect(&config, ClientConfig::recommended(&config))
                .await
                .err()
        }
    });
    tokio::time::timeout(IO_TIMEOUT, server.connect())
        .await
        .expect("the client never came")
        .unwrap();
    let frame = tokio::time::timeout(
        IO_TIMEOUT,
        wire::read_frame(&mut server, config.max_frame_bytes),
    )
    .await
    .expect("the client sent no whole frame")
    .unwrap()
    .expect("the client closed before its first frame");
    let Frame::Json(payload) = frame else {
        panic!("the first frame is not JSON: {frame:?}");
    };
    let request = Request::parse(&payload).unwrap();
    match request.call {
        Call::Hello(params) => assert_eq!(params.contract, wire::CONTRACT),
        other => panic!("the first call is not host.hello: {other:?}"),
    }
    // The client waits for an answer that never comes; it is not the subject here.
    assert!(!client.is_finished(), "{:?}", client.await);
    client.abort();
}
