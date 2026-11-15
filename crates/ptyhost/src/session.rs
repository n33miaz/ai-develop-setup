//! One terminal session: pumps output into the replay ring and fans it out to subscribers.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use tokio::sync::{mpsc, watch};

use crate::backend::{PtyControl, PtyParts, PtyWaiter};
use crate::config::HostConfig;
use crate::replay::ReplayRing;
use crate::types::{
    ExitStatus, OutputChunk, PtyError, ReplayFrom, Seq, SubscriptionEnd, TerminalSize,
};

/// A panic on another thread must not take the whole host down with a poisoned lock.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Shared {
    ring: ReplayRing,
    subscribers: Vec<mpsc::Sender<OutputChunk>>,
    ended: bool,
    closed: bool,
}

/// Replay followed by live output, in `seq` order with no gap and no repeat.
///
/// `recv` returns `None` when the session output ended or when this subscriber fell too far
/// behind; in both cases subscribing again from the last seen `seq + 1` loses nothing.
pub struct Subscription {
    backlog: VecDeque<OutputChunk>,
    live: mpsc::Receiver<OutputChunk>,
    shared: Arc<Mutex<Shared>>,
    exit: watch::Receiver<Option<ExitStatus>>,
    oldest_seq: Seq,
    next_seq: Seq,
    /// The number the next delivered chunk must have.
    expected: u64,
}

impl Subscription {
    pub async fn recv(&mut self) -> Option<OutputChunk> {
        let chunk = match self.backlog.pop_front() {
            Some(chunk) => Some(chunk),
            None => self.live.recv().await,
        }?;
        self.expected = chunk.seq.0 + 1;
        Some(chunk)
    }

    /// First chunk the replay ring held when this subscription was made.
    pub fn oldest_seq(&self) -> Seq {
        self.oldest_seq
    }

    /// The number the next chunk was going to get when this subscription was made.
    pub fn next_seq(&self) -> Seq {
        self.next_seq
    }

    /// Last chunk delivered so far, or the one right before the starting point.
    pub fn last_seq(&self) -> u64 {
        self.expected - 1
    }

    /// Why `recv` returned `None`. Waits for the exit status when the process is on its way out.
    pub async fn end(&mut self) -> SubscriptionEnd {
        let (closed, drained) = {
            let shared = lock(&self.shared);
            let drained = shared.ended && self.expected == shared.ring.next_seq().0;
            (shared.closed, drained)
        };
        if closed {
            return SubscriptionEnd::SessionClosed;
        }
        if !drained {
            return SubscriptionEnd::Lagged;
        }
        match self.exit.wait_for(Option::is_some).await.map(|exit| *exit) {
            Ok(Some(status)) => SubscriptionEnd::Exited(status),
            _ => SubscriptionEnd::SessionClosed,
        }
    }
}

pub(crate) struct Session {
    shared: Arc<Mutex<Shared>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    control: Arc<Mutex<Box<dyn PtyControl>>>,
    exit: watch::Receiver<Option<ExitStatus>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    queue_chunks: usize,
}

impl Session {
    pub(crate) fn start(parts: PtyParts, config: &HostConfig) -> Result<Self, PtyError> {
        let PtyParts {
            reader,
            writer,
            control,
            waiter,
        } = parts;
        let shared = Arc::new(Mutex::new(Shared {
            ring: ReplayRing::new(config.replay_capacity_bytes),
            subscribers: Vec::new(),
            ended: false,
            closed: false,
        }));
        let control = Arc::new(Mutex::new(control));
        let (exit_tx, exit) = watch::channel(None);
        let session = Self {
            shared: Arc::clone(&shared),
            writer: Mutex::new(Some(writer)),
            control: Arc::clone(&control),
            exit,
            threads: Mutex::new(Vec::new()),
            queue_chunks: config.subscriber_queue_chunks,
        };

        // If a thread cannot start, dropping `session` kills the tree and releases the terminal.
        let buffer_bytes = config.read_buffer_bytes;
        let pump = thread::Builder::new()
            .name("pty-pump".to_owned())
            .spawn(move || pump_output(reader, &shared, buffer_bytes))
            .map_err(|err| PtyError::Spawn(err.to_string()))?;
        lock(&session.threads).push(pump);
        let wait = thread::Builder::new()
            .name("pty-wait".to_owned())
            .spawn(move || wait_exit(waiter, &control, &exit_tx))
            .map_err(|err| PtyError::Spawn(err.to_string()))?;
        lock(&session.threads).push(wait);
        Ok(session)
    }

    pub(crate) fn subscribe(&self, from: ReplayFrom) -> Result<Subscription, PtyError> {
        // Snapshot and registration under one lock: nothing can slip between replay and live.
        let mut shared = lock(&self.shared);
        let backlog: VecDeque<_> = shared.ring.replay(from)?.into();
        let oldest_seq = shared.ring.oldest_seq();
        let next_seq = shared.ring.next_seq();
        let (tx, live) = mpsc::channel(self.queue_chunks);
        if !shared.ended {
            shared.subscribers.push(tx);
        }
        Ok(Subscription {
            expected: backlog.front().map_or(next_seq.0, |chunk| chunk.seq.0),
            backlog,
            live,
            shared: Arc::clone(&self.shared),
            exit: self.exit.clone(),
            oldest_seq,
            next_seq,
        })
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> Result<(), PtyError> {
        let mut writer = lock(&self.writer);
        let writer = writer.as_mut().ok_or(PtyError::SessionEnded)?;
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .map_err(|err| PtyError::Io(err.to_string()))
    }

    pub(crate) fn resize(&self, size: TerminalSize) -> Result<(), PtyError> {
        lock(&self.control).resize(size)
    }

    pub(crate) fn tree_process_ids(&self) -> Vec<u32> {
        lock(&self.control).tree_process_ids()
    }

    pub(crate) fn exit_status(&self) -> Option<ExitStatus> {
        *self.exit.borrow()
    }

    /// Oldest chunk still in the replay ring and the number the next one will get.
    pub(crate) fn replay_bounds(&self) -> (Seq, Seq) {
        let shared = lock(&self.shared);
        (shared.ring.oldest_seq(), shared.ring.next_seq())
    }

    pub(crate) async fn wait_exit(&self) -> Result<ExitStatus, PtyError> {
        let mut exit = self.exit.clone();
        let status = exit
            .wait_for(Option::is_some)
            .await
            .map_err(|_| PtyError::SessionEnded)?;
        (*status).ok_or(PtyError::SessionEnded)
    }

    /// Kills the process tree and returns once all output is in the ring and the threads are gone.
    pub(crate) async fn close(&self) -> Result<ExitStatus, PtyError> {
        lock(&self.shared).closed = true;
        lock(&self.control).kill_tree();
        lock(&self.writer).take();
        let threads = std::mem::take(&mut *lock(&self.threads));
        let joined = tokio::task::spawn_blocking(move || {
            for thread in threads {
                let _ = thread.join();
            }
        });
        joined.await.map_err(|err| PtyError::Io(err.to_string()))?;
        self.exit_status().ok_or(PtyError::SessionEnded)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let mut control = lock(&self.control);
        control.kill_tree();
        control.release();
    }
}

fn pump_output(mut reader: Box<dyn Read + Send>, shared: &Mutex<Shared>, buffer_bytes: usize) {
    let mut buffer = vec![0_u8; buffer_bytes];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let mut shared = lock(shared);
        for chunk in shared.ring.push(&buffer[..read]) {
            // A full or closed queue drops the subscriber; it can come back through replay.
            shared
                .subscribers
                .retain(|subscriber| subscriber.try_send(chunk.clone()).is_ok());
        }
    }
    let mut shared = lock(shared);
    shared.ended = true;
    shared.subscribers.clear();
}

fn wait_exit(
    waiter: Box<dyn PtyWaiter>,
    control: &Mutex<Box<dyn PtyControl>>,
    exit: &watch::Sender<Option<ExitStatus>>,
) {
    let status = waiter.wait();
    {
        // The root is gone: whatever it left behind goes too, and the reader gets its end of file.
        let mut control = lock(control);
        control.release();
        control.kill_tree();
    }
    exit.send_replace(Some(status));
}
