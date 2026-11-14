//! The real backend with harmless system commands (ConPTY on Windows, a Unix pty elsewhere).

mod common;

use std::sync::Arc;

use ads_ptyhost::{ExitStatus, HostConfig, PortablePtyBackend, PtyHost, ReplayFrom};
use common::{assert_contiguous, next, shell, text};

fn host() -> PtyHost {
    PtyHost::new(Arc::new(PortablePtyBackend), HostConfig::defaults()).unwrap()
}

#[tokio::test]
async fn short_command_output_is_captured_until_exit() {
    let host = host();
    let id = host.open(&shell("echo pty-marker-1")).unwrap();
    let mut subscription = host.subscribe(id, ReplayFrom::Oldest).unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk) = next(&mut subscription).await {
        chunks.push(chunk);
    }
    assert_contiguous(&chunks);
    assert!(
        text(&chunks).contains("pty-marker-1"),
        "{:?}",
        text(&chunks)
    );
    assert_eq!(host.wait_exit(id).await.unwrap(), ExitStatus { code: 0 });

    // The replay survives the process: a late client still gets the whole output.
    let mut late = host.subscribe(id, ReplayFrom::Oldest).unwrap();
    let mut replayed = Vec::new();
    while let Some(chunk) = next(&mut late).await {
        replayed.push(chunk);
    }
    assert_eq!(replayed, chunks);
    host.close(id).await.unwrap();
}

#[tokio::test]
async fn exit_code_of_the_root_process_is_reported() {
    let host = host();
    let id = host.open(&shell("exit 3")).unwrap();
    assert_eq!(host.wait_exit(id).await.unwrap(), ExitStatus { code: 3 });
    host.close(id).await.unwrap();
}

#[tokio::test]
async fn child_sees_only_the_environment_of_the_spec() {
    // The variable exists in this process (the test harness sets it) but is not in the spec.
    assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
    #[cfg(windows)]
    let command = "echo [%CARGO_MANIFEST_DIR%] [%ADS_TEST_MARK%]";
    #[cfg(not(windows))]
    let command = "echo [${CARGO_MANIFEST_DIR:-unset}] [$ADS_TEST_MARK]";
    let mut spec = shell(command);
    spec.env
        .push(("ADS_TEST_MARK".to_owned(), "from-spec".to_owned()));

    let host = host();
    let id = host.open(&spec).unwrap();
    let mut subscription = host.subscribe(id, ReplayFrom::Oldest).unwrap();
    let mut chunks = Vec::new();
    while let Some(chunk) = next(&mut subscription).await {
        chunks.push(chunk);
    }
    let output = text(&chunks);
    assert!(output.contains("[from-spec]"), "{output:?}");
    #[cfg(windows)]
    assert!(output.contains("[%CARGO_MANIFEST_DIR%]"), "{output:?}");
    #[cfg(not(windows))]
    assert!(output.contains("[unset]"), "{output:?}");
    host.close(id).await.unwrap();
}
