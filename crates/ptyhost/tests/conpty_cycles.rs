//! Windows only: open and close real ConPTY sessions in a loop and prove nothing is left behind.
//! Alone in this file on purpose: handle and child-process counts are per test binary.
#![cfg(windows)]

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ads_ptyhost::win::{ProcessProbe, own_handle_count, process_table};
use ads_ptyhost::{HostConfig, PortablePtyBackend, PtyHost, ReplayFrom};
use common::{STEP_TIMEOUT, assert_contiguous, next, open, shell, text};

const CYCLES: usize = 20;
/// Cycles run before the baseline is taken, so one-time setup (thread pool, loaded libraries)
/// is not counted as a leak.
const WARMUP_CYCLES: usize = 2;
/// A shell that starts a second process, so every session has a tree to kill.
const TREE_COMMAND: &str = "ping -n 60 127.0.0.1";
const SETTLE_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const STABLE_SAMPLES: usize = 3;

fn own_children() -> HashSet<u32> {
    let me = std::process::id();
    process_table()
        .unwrap()
        .into_iter()
        .filter(|entry| entry.parent_process_id == me)
        .map(|entry| entry.process_id)
        .collect()
}

async fn host_output(host: &PtyHost, id: &ads_ptyhost::SessionId) -> Vec<ads_ptyhost::OutputChunk> {
    let mut all = host.subscribe(id, ReplayFrom::Oldest).unwrap();
    let mut chunks = Vec::new();
    while let Ok(Some(chunk)) = tokio::time::timeout(POLL_INTERVAL, all.recv()).await {
        chunks.push(chunk);
    }
    chunks
}

/// One session: wait for output and for the whole tree, close, and return the tree it had.
async fn cycle(host: &PtyHost) -> Vec<ProcessProbe> {
    let id = open(host, &shell(TREE_COMMAND));
    let mut subscription = host.subscribe(&id, ReplayFrom::Oldest).unwrap();
    let mut chunks = vec![next(&mut subscription).await.expect("session ended early")];

    let deadline = Instant::now() + STEP_TIMEOUT;
    let mut tree = host.tree_process_ids(&id).unwrap();
    while tree.len() < 2 {
        assert!(
            Instant::now() < deadline,
            "tree never grew past the shell: {tree:?}; output so far: {:?}",
            text(&host_output(host, &id).await)
        );
        tokio::time::sleep(POLL_INTERVAL).await;
        tree = host.tree_process_ids(&id).unwrap();
    }
    assert_eq!(host.exit_status(&id).unwrap(), None);

    let tree = tree
        .into_iter()
        .map(|pid| {
            ProcessProbe::open(pid).unwrap_or_else(|err| panic!("open process {pid}: {err}"))
        })
        .collect();

    host.close(&id).await.unwrap();

    while let Some(chunk) = next(&mut subscription).await {
        chunks.push(chunk);
    }
    assert_contiguous(&chunks);
    assert!(host.session_ids().is_empty());
    tree
}

async fn settle(
    mut trees: Vec<ProcessProbe>,
    children_before: &HashSet<u32>,
    handle_limit: u32,
) -> u32 {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let mut previous_count = None;
    let mut stable_samples = 0;
    let (leftover_children, handle_count) = loop {
        // Drop probes for exited processes before measuring the handles owned by the test.
        trees.retain(|process| process.is_alive().unwrap());
        let leftover_children: Vec<u32> = own_children()
            .difference(children_before)
            .copied()
            .collect();
        let handle_count = own_handle_count().unwrap();
        if trees.is_empty() && leftover_children.is_empty() && handle_count <= handle_limit {
            stable_samples = if previous_count == Some(handle_count) {
                stable_samples + 1
            } else {
                1
            };
            previous_count = Some(handle_count);
        } else {
            stable_samples = 0;
            previous_count = None;
        }
        if stable_samples == STABLE_SAMPLES || Instant::now() >= deadline {
            break (leftover_children, handle_count);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    };
    let leftover_tree: Vec<_> = trees.iter().map(ProcessProbe::id).collect();
    assert!(
        leftover_tree.is_empty(),
        "session processes still alive: {leftover_tree:?}"
    );
    assert!(
        leftover_children.is_empty(),
        "child processes still alive: {leftover_children:?}"
    );
    assert!(
        handle_count <= handle_limit,
        "handle count grew from {handle_limit} to {handle_count}"
    );
    assert_eq!(
        stable_samples, STABLE_SAMPLES,
        "handle count did not settle"
    );
    handle_count
}

#[tokio::test]
async fn twenty_cycles_leave_no_process_and_no_handle() {
    let host = PtyHost::new(Arc::new(PortablePtyBackend), HostConfig::defaults()).unwrap();
    let children_before = own_children();
    let mut warmup = Vec::new();
    for _ in 0..WARMUP_CYCLES {
        warmup.extend(cycle(&host).await);
    }
    let handles_before = settle(warmup, &children_before, u32::MAX).await;

    let started = Instant::now();
    let mut trees = Vec::new();
    for _ in 0..CYCLES {
        trees.extend(cycle(&host).await);
    }
    let process_count = trees.len();
    let elapsed = started.elapsed();
    let handles_after = settle(trees, &children_before, handles_before).await;
    println!(
        "pty cycles: {CYCLES} in {} ms; session processes seen: {process_count}; \
         handles before: {handles_before}, after: {handles_after}",
        elapsed.as_millis(),
    );
}
