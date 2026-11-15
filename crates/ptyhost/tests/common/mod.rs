//! Helpers for the tests that start a real, harmless system command.
#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ads_ptyhost::{OutputChunk, PtyHost, SessionId, SpawnSpec, Subscription};

pub mod client;
pub mod gated;

pub const STEP_TIMEOUT: Duration = Duration::from_secs(30);

/// Variables a system command needs to run at all; everything else stays out of the child.
#[cfg(windows)]
const ENV_ALLOWLIST: &[&str] = &["SystemRoot", "windir", "ComSpec", "PATH", "PATHEXT"];
#[cfg(not(windows))]
const ENV_ALLOWLIST: &[&str] = &["PATH"];

/// Opens a session under a name no other session of this test binary has.
pub fn open(host: &PtyHost, spec: &SpawnSpec) -> SessionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = SessionId::new(&format!("s{:03}", NEXT.fetch_add(1, Ordering::Relaxed))).unwrap();
    host.open(&id, spec).unwrap();
    id
}

pub fn allowed_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(name, _)| {
            ENV_ALLOWLIST
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(name))
        })
        .collect()
}

/// A shell running one command line.
pub fn shell(command_line: &str) -> SpawnSpec {
    #[cfg(windows)]
    let (program, flags) = ("cmd.exe", ["/d", "/c"].as_slice());
    #[cfg(not(windows))]
    let (program, flags) = ("/bin/sh", ["-c"].as_slice());
    SpawnSpec {
        program: program.to_owned(),
        args: flags
            .iter()
            .map(|flag| (*flag).to_owned())
            .chain([command_line.to_owned()])
            .collect(),
        env: allowed_env(),
        ..SpawnSpec::default()
    }
}

pub async fn next(subscription: &mut Subscription) -> Option<OutputChunk> {
    tokio::time::timeout(STEP_TIMEOUT, subscription.recv())
        .await
        .expect("no output within the step timeout")
}

pub fn text(chunks: &[OutputChunk]) -> String {
    let bytes: Vec<u8> = chunks
        .iter()
        .flat_map(|c| c.bytes.iter().copied())
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn assert_contiguous(chunks: &[OutputChunk]) {
    for (index, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.seq.0, index as u64 + 1, "gap or repeat in the stream");
    }
}
