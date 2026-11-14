//! Windows only: open and close real ConPTY sessions in a loop and prove nothing is left behind.
//! Alone in this file on purpose: handle and child-process counts are per test binary.
#![cfg(windows)]

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ads_ptyhost::win::{own_handle_count, process_table};
use ads_ptyhost::{HostConfig, PortablePtyBackend, PtyHost, ReplayFrom};
use common::{STEP_TIMEOUT, assert_contiguous, next, shell, text};

const CYCLES: usize = 20;
/// Cycles run before the baseline is taken, so one-time setup (thread pool, loaded libraries)
/// is not counted as a leak.
const WARMUP_CYCLES: usize = 2;
/// A shell that starts a second process, so every session has a tree to kill.
const TREE_COMMAND: &str = "ping -n 60 127.0.0.1";
const SETTLE_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

fn own_children() -> HashSet<u32> {
    let me = std::process::id();
    process_table()
        .unwrap()
        .into_iter()
        .filter(|entry| entry.parent_process_id == me)
        .map(|entry| entry.process_id)
        .collect()
}

fn alive(process_ids: &HashSet<u32>) -> Vec<u32> {
    let table: HashSet<u32> = process_table()
        .unwrap()
        .into_iter()
        .map(|entry| entry.process_id)
        .collect();
    let mut alive: Vec<u32> = process_ids.intersection(&table).copied().collect();
    alive.sort_unstable();
    alive
}

async fn host_output(host: &PtyHost, id: ads_ptyhost::SessionId) -> Vec<ads_ptyhost::OutputChunk> {
    let mut all = host.subscribe(id, ReplayFrom::Oldest).unwrap();
    let mut chunks = Vec::new();
    while let Ok(Some(chunk)) = tokio::time::timeout(POLL_INTERVAL, all.recv()).await {
        chunks.push(chunk);
    }
    chunks
}

/// One session: wait for output and for the whole tree, close, and return the tree it had.
async fn cycle(host: &PtyHost) -> HashSet<u32> {
    let id = host.open(&shell(TREE_COMMAND)).unwrap();
    let mut subscription = host.subscribe(id, ReplayFrom::Oldest).unwrap();
    let mut chunks = vec![next(&mut subscription).await.expect("session ended early")];

    let deadline = Instant::now() + STEP_TIMEOUT;
    let mut tree = host.tree_process_ids(id).unwrap();
    while tree.len() < 2 {
        assert!(
            Instant::now() < deadline,
            "tree never grew past the shell: {tree:?}; output so far: {:?}",
            text(&host_output(host, id).await)
        );
        tokio::time::sleep(POLL_INTERVAL).await;
        tree = host.tree_process_ids(id).unwrap();
    }
    assert_eq!(host.exit_status(id).unwrap(), None);

    host.close(id).await.unwrap();

    while let Some(chunk) = next(&mut subscription).await {
        chunks.push(chunk);
    }
    assert_contiguous(&chunks);
    assert!(host.session_ids().is_empty());
    tree.into_iter().collect()
}

#[tokio::test]
async fn twenty_cycles_leave_no_process_and_no_handle() {
    let host = PtyHost::new(Arc::new(PortablePtyBackend), HostConfig::defaults()).unwrap();
    let children_before = own_children();
    for _ in 0..WARMUP_CYCLES {
        cycle(&host).await;
    }
    let handles_before = own_handle_count().unwrap();

    let started = Instant::now();
    let mut trees = HashSet::new();
    for _ in 0..CYCLES {
        let tree = cycle(&host).await;
        assert!(tree.len() >= 2);
        trees.extend(tree);
    }
    let elapsed = started.elapsed();
    assert!(trees.len() >= CYCLES * 2, "process ids: {trees:?}");

    // The console host of each session exits on its own right after the terminal closes.
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let (leftover_tree, leftover_children, handles_after) = loop {
        let leftover_tree = alive(&trees);
        let leftover_children: Vec<u32> = own_children()
            .difference(&children_before)
            .copied()
            .collect();
        let handles_after = own_handle_count().unwrap();
        let clean = leftover_tree.is_empty()
            && leftover_children.is_empty()
            && handles_after <= handles_before;
        if clean || Instant::now() >= deadline {
            break (leftover_tree, leftover_children, handles_after);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    };

    println!(
        "pty cycles: {CYCLES} in {} ms; session processes seen: {}; left alive: {}; \
         child processes left: {}; handles before: {handles_before}, after: {handles_after}",
        elapsed.as_millis(),
        trees.len(),
        leftover_tree.len(),
        leftover_children.len(),
    );
    assert!(
        leftover_tree.is_empty(),
        "session processes still alive: {leftover_tree:?}"
    );
    assert!(
        leftover_children.is_empty(),
        "child processes still alive: {leftover_children:?}"
    );
    assert!(
        handles_after <= handles_before,
        "handle count grew from {handles_before} to {handles_after}"
    );
}
