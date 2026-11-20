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

/// A real pipe, served by this user and read by the real queries, refused because the check
/// compares it with another identity: the server sees the client come and go with no byte, so
/// `host.hello` never left. The same pipe checked as the current user receives the hello.
#[tokio::test]
async fn real_pipe_refused_by_the_owner_check_receives_no_hello() {
    use ads_ptyhost::win::{
        LocalUser, check_pipe_server, check_pipe_server_as, create_user_pipe, current_user_sid,
    };
    use tokio::io::AsyncReadExt;

    const IO_TIMEOUT: Duration = Duration::from_secs(5);
    let stranger = LocalUser {
        user: "S-1-5-21-1000-2000-3000-1002".to_owned(),
        default_owner: "S-1-5-21-1000-2000-3000-1002".to_owned(),
    };
    let endpoint = format!(r"\\.\pipe\ads-daemon-owner-{}", std::process::id());
    let config = HostConfig::from_toml_str(&format!("channel_path = '{endpoint}'\n")).unwrap();
    let sid = current_user_sid().unwrap();

    let mut server = create_user_pipe(&endpoint, &sid, true).unwrap();
    let refused = {
        let mut waiting = std::pin::pin!(server.connect());
        // Waiting before the client opens, so a client already gone cannot fail the wait.
        tokio::select! {
            biased;
            early = &mut waiting => panic!("connected with no client: {early:?}"),
            () = std::future::ready(()) => {}
        }
        // A client that got through would wait for the answer to its hello: it runs apart, so
        // that case fails on the server's read instead of hanging the test.
        let client = tokio::spawn({
            let config = config.clone();
            async move {
                PtyClient::connect_checked(&config, ClientConfig::recommended(&config), |pipe| {
                    check_pipe_server_as(&stranger, pipe)
                })
                .await
                .err()
            }
        });
        tokio::time::timeout(IO_TIMEOUT, waiting)
            .await
            .expect("the server never saw the client")
            .unwrap();
        client
    };
    let mut received = Vec::new();
    let read = tokio::time::timeout(IO_TIMEOUT, server.read_to_end(&mut received)).await;
    refused.abort();
    read.unwrap_or_else(|_| {
        panic!(
            "the client left the pipe open with {} bytes",
            received.len()
        )
    })
    .unwrap();
    let refused = refused.await.unwrap();
    assert_eq!(refused, Some(ClientError::NotOwnedByUser));
    assert!(
        received.is_empty(),
        "{} bytes reached the pipe",
        received.len()
    );

    // Control: the same kind of pipe, checked as the current user, gets the hello.
    let mut server = create_user_pipe(&endpoint, &sid, false).unwrap();
    let accepted = tokio::spawn({
        let config = config.clone();
        async move {
            PtyClient::connect_checked(
                &config,
                ClientConfig::recommended(&config),
                check_pipe_server,
            )
            .await
            .err()
        }
    });
    tokio::time::timeout(IO_TIMEOUT, server.connect())
        .await
        .expect("the accepted client never came")
        .unwrap();
    let mut first = [0_u8; 1];
    let read = tokio::time::timeout(IO_TIMEOUT, server.read(&mut first))
        .await
        .expect("the accepted client sent nothing")
        .unwrap();
    assert_eq!(read, 1);
    // The client waits for an answer that never comes; it is not the subject here.
    assert!(!accepted.is_finished(), "{:?}", accepted.await);
    accepted.abort();
}
