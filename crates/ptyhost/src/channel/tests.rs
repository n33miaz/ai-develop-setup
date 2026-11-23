use super::*;

#[cfg(windows)]
fn test_endpoint(label: &str) -> String {
    format!(r"\\.\pipe\ads-ptyhost-unit-{}-{label}", std::process::id())
}

#[test]
fn explicit_path_wins_over_the_derived_one() {
    let config = HostConfig::from_toml_str("channel_path = 'explicit'").unwrap();
    assert_eq!(endpoint(&config).unwrap(), "explicit");
}

#[cfg(windows)]
#[test]
fn derived_pipe_name_carries_the_user_identifier() {
    let derived = endpoint(&HostConfig::defaults()).unwrap();
    let sid = crate::win::current_user_sid().unwrap();
    assert!(sid.starts_with("S-1-"), "{sid}");
    assert_eq!(derived, format!(r"\\.\pipe\ads-ptyhost.{sid}"));
}

#[cfg(windows)]
#[tokio::test]
async fn pipe_grants_access_to_the_current_user_only() {
    use std::os::windows::io::AsRawHandle;

    let mut listener = Listener::bind(&test_endpoint("acl")).unwrap();
    let sid = crate::win::current_user_sid().unwrap();
    let check = |stream: &Stream| {
        let rules = crate::win::access_list_text(stream.as_raw_handle()).unwrap();
        // Protected list (no inherited entries) with exactly one entry, for this user.
        assert!(rules.starts_with("D:P(A;;"), "{rules}");
        // The system writes the built-in administrator and guest accounts by their alias.
        let who = match sid.rsplit('-').next() {
            Some("500") => "LA",
            Some("501") => "LG",
            _ => sid.as_str(),
        };
        assert!(
            rules.ends_with(&format!(";;;{sid})")) || rules.ends_with(&format!(";;;{who})")),
            "{rules}"
        );
        assert_eq!(rules.matches('(').count(), 1, "{rules}");
    };
    check(&listener.next);

    // Every later instance gets the same rule, not the system default.
    let client = tokio::net::windows::named_pipe::ClientOptions::new()
        .open(listener.endpoint())
        .unwrap();
    let accepted = listener.accept().await.unwrap();
    check(&accepted);
    check(&listener.next);
    drop(client);
}

#[cfg(windows)]
#[tokio::test]
async fn client_accepts_the_pipe_of_its_own_host() {
    use std::os::windows::io::AsRawHandle;

    let listener = Listener::bind(&test_endpoint("own")).unwrap();
    let client = connect(listener.endpoint()).await.unwrap();
    let local = crate::win::local_user().unwrap();
    let server = crate::win::pipe_server(client.as_raw_handle()).unwrap();
    assert_eq!(server.process_user, local.user);
    assert!(
        server.object_owner == local.user || server.object_owner == local.default_owner,
        "{server:?} {local:?}"
    );
}

/// What the server of a fresh pipe sees when a client connects with `check` as the owner
/// check and, if accepted, sends `sent` and leaves: whether the client was refused, and every
/// byte that reached the server before the end of the stream.
#[cfg(windows)]
async fn serve_one_client(
    label: &str,
    check: impl FnOnce(std::os::windows::io::RawHandle) -> io::Result<()>,
    sent: &[u8],
) -> (bool, Vec<u8>) {
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let endpoint = test_endpoint(label);
    let sid = crate::win::current_user_sid().unwrap();
    let mut server = crate::win::create_user_pipe(&endpoint, &sid, true).unwrap();
    let refused = {
        let mut waiting = std::pin::pin!(server.connect());
        // The server starts waiting before the client opens: a client gone before the wait
        // began would fail the wait and hide what reached the pipe.
        tokio::select! {
            biased;
            early = &mut waiting => panic!("connected with no client: {early:?}"),
            () = std::future::ready(()) => {}
        }
        let refused = match connect_checked(&endpoint, check) {
            Ok(mut client) => {
                client.write_all(sent).await.unwrap();
                false
            }
            Err(ConnectError::NotOwnedByUser) => true,
            Err(other) => panic!("the pipe did not open: {other:?}"),
        };
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("the server never saw the client")
            .expect("the server wait failed");
        refused
    };
    let mut received = Vec::new();
    // The client end closes: the stream ends with no error.
    tokio::time::timeout(Duration::from_secs(5), server.read_to_end(&mut received))
        .await
        .expect("the client left the pipe open")
        .expect("the stream did not end cleanly");
    (refused, received)
}

#[cfg(windows)]
#[tokio::test]
async fn accepted_check_lets_the_bytes_through() {
    let (refused, received) = serve_one_client("accepted", |_| Ok(()), b"hello").await;
    assert!(!refused);
    assert_eq!(received, b"hello");
}

#[cfg(windows)]
#[tokio::test]
async fn refused_check_closes_the_pipe_before_any_byte() {
    let mut checked = false;
    let (refused, received) = serve_one_client(
        "refused",
        |_| {
            checked = true;
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        },
        b"hello",
    )
    .await;
    assert!(checked);
    assert!(refused);
    assert!(received.is_empty(), "{} bytes got through", received.len());
}

#[cfg(windows)]
#[tokio::test]
async fn real_queries_refuse_a_pipe_for_another_identity() {
    // The host's own pipe, read by the real queries, checked as if the client were someone
    // else: the server is refused and sees no byte.
    let stranger = crate::win::LocalUser {
        user: "S-1-5-21-1000-2000-3000-1002".to_owned(),
        default_owner: "S-1-5-21-1000-2000-3000-1002".to_owned(),
    };
    let (refused, received) = serve_one_client(
        "stranger",
        |pipe| crate::win::check_pipe_server_as(&stranger, pipe),
        b"hello",
    )
    .await;
    assert!(refused);
    assert!(received.is_empty(), "{} bytes got through", received.len());

    let (refused, received) =
        serve_one_client("self", crate::win::check_pipe_server, b"hello").await;
    assert!(!refused);
    assert_eq!(received, b"hello");
}

#[cfg(windows)]
#[tokio::test]
async fn something_at_the_endpoint_that_is_not_a_pipe_is_refused_untouched() {
    // The closest the test gets to a stranger's server without a second account: the
    // endpoint opens, but nobody can be shown to serve it.
    let path = std::env::temp_dir().join(format!("ads-ptyhost-not-a-pipe-{}", std::process::id()));
    std::fs::write(&path, b"").unwrap();
    let result = connect(path.to_str().unwrap()).await;
    let refused = matches!(result, Err(ConnectError::NotOwnedByUser));
    drop(result);
    // The runtime closes the handle once it has seen the end of its pending read.
    let mut written = std::fs::read(&path);
    for _ in 0..100 {
        if written.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        written = std::fs::read(&path);
    }
    let written = written.unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(refused);
    assert!(written.is_empty());
}

#[cfg(windows)]
#[tokio::test]
async fn a_second_host_cannot_take_the_same_pipe() {
    let endpoint = test_endpoint("twice");
    let _first = Listener::bind(&endpoint).unwrap();
    assert!(Listener::bind(&endpoint).is_err());
}

/// Tells the child process started by `restricted_server` what to do: `<part>|<endpoint>`.
#[cfg(windows)]
const RESTRICTED_VARIABLE: &str = "ADS_TEST_RESTRICTED_PIPE";
#[cfg(windows)]
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Not a test: the child process of `restricted_server`. It creates the pipe and starts
/// waiting for a client, closes its own process or token to everyone but the system
/// account, as a server of another account would be, then says it is ready, serves one
/// client and reports how many bytes it received.
#[cfg(windows)]
#[tokio::test]
#[ignore = "helper run in a child process by the restricted server tests"]
async fn restricted_server_helper() {
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
    let sid = crate::win::current_user_sid().unwrap();
    let mut server = crate::win::create_user_pipe(endpoint, &sid, true).unwrap();
    {
        let mut waiting = std::pin::pin!(server.connect());
        // The wait starts before the client is told to come: a client already gone could
        // not fail it.
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
    tokio::time::timeout(HELPER_TIMEOUT, server.read_to_end(&mut received))
        .await
        .expect("the client left the pipe open")
        .unwrap();
    println!("@@received={}@@", received.len());
}

/// A pipe served by a child process of this user whose process or token nobody else may
/// open: the client's own check refuses it at that query, and the child receives no byte.
/// The child is killed and reaped on every way out, failures included.
#[cfg(windows)]
async fn restricted_server(part: &str, expected: crate::win::PipeQuery) {
    let endpoint = test_endpoint(&format!("restricted-{part}"));
    let mut helper = ads_testkit::HelperProcess::start(
        "channel::tests::restricted_server_helper",
        RESTRICTED_VARIABLE,
        &format!("{part}|{endpoint}"),
    )
    .unwrap();
    assert!(
        helper.wait_for("@@ready@@", HELPER_TIMEOUT).is_some(),
        "the helper never got ready"
    );

    if part == "process" && crate::win::ProcessProbe::open(helper.id()).is_ok() {
        // This process opens the closed process anyway: it holds `SeDebugPrivilege` enabled,
        // as an elevated administrator may (the Windows CI runner does). The stand-in for
        // another account does not hold here; what holds is that the check reads the real
        // server, of this same user, and accepts it. The refusal at `OpenProcess` is still
        // covered by the injected failure in `win::tests`.
        eprintln!(
            "not applicable here: this process opens any process (SeDebugPrivilege); \
             checking that the real server is read and accepted instead"
        );
        let client = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&endpoint)
            .unwrap();
        {
            use std::os::windows::io::AsRawHandle;
            crate::win::check_pipe_server(client.as_raw_handle()).unwrap();
        }
        drop(client);
        let (helper, received) = tokio::task::spawn_blocking(move || {
            let received = helper.wait_for("@@received=", HELPER_TIMEOUT);
            (helper, received)
        })
        .await
        .unwrap();
        assert_eq!(received.as_deref(), Some("0"));
        assert!(
            helper
                .finish(HELPER_TIMEOUT)
                .expect("the helper did not end")
                .success()
        );
        return;
    }

    let mut failed = None;
    let result = connect_checked(&endpoint, |pipe| {
        let checked = crate::win::check_pipe_server(pipe);
        failed = checked
            .as_ref()
            .err()
            .and_then(crate::win::PipeQueryFailed::of);
        checked
    });
    assert!(
        matches!(result, Err(ConnectError::NotOwnedByUser)),
        "{result:?}"
    );
    assert_eq!(failed, Some(expected));
    drop(result);

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

#[cfg(windows)]
#[tokio::test]
async fn a_server_whose_process_cannot_be_opened_is_refused() {
    restricted_server("process", crate::win::PipeQuery::OpenProcess).await;
}

#[cfg(windows)]
#[tokio::test]
async fn a_server_whose_token_cannot_be_opened_is_refused() {
    restricted_server("token", crate::win::PipeQuery::OpenToken).await;
}
