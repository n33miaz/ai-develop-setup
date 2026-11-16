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
