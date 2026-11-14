//! Deterministic terminal with no real process, driven by the test through a handle.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use crate::backend::{PtyBackend, PtyControl, PtyParts, PtyWaiter};
use crate::session::lock;
use crate::types::{ExitStatus, PtyError, SpawnSpec, TerminalSize};

/// Exit code reported when the fake terminal is killed instead of exiting by itself.
const KILLED_EXIT_CODE: u32 = 1;

#[derive(Default)]
pub struct FakeBackend {
    echo: bool,
    terminals: Mutex<Vec<FakeTerminal>>,
}

impl FakeBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every byte written to a terminal comes back as output.
    pub fn with_echo() -> Self {
        Self {
            echo: true,
            ..Self::default()
        }
    }

    /// Handles of every terminal spawned so far, in spawn order.
    pub fn terminals(&self) -> Vec<FakeTerminal> {
        lock(&self.terminals).clone()
    }
}

impl PtyBackend for FakeBackend {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtyParts, PtyError> {
        if spec.program.is_empty() {
            return Err(PtyError::Spawn("empty program".to_owned()));
        }
        let (out_tx, out_rx) = channel();
        let (exit_tx, exit_rx) = channel();
        let terminal = FakeTerminal {
            inner: Arc::new(Inner {
                spec: spec.clone(),
                echo: self.echo,
                output: Mutex::new(Some(out_tx)),
                exit: Mutex::new(Some(exit_tx)),
                input: Mutex::new(Vec::new()),
                size: Mutex::new(spec.size),
                killed: AtomicBool::new(false),
            }),
        };
        lock(&self.terminals).push(terminal.clone());
        Ok(PtyParts {
            reader: Box::new(FakeReader {
                output: out_rx,
                pending: Vec::new(),
            }),
            writer: Box::new(terminal.clone()),
            control: Box::new(terminal),
            waiter: Box::new(FakeWaiter(exit_rx)),
        })
    }
}

struct Inner {
    spec: SpawnSpec,
    echo: bool,
    output: Mutex<Option<Sender<Vec<u8>>>>,
    exit: Mutex<Option<Sender<u32>>>,
    input: Mutex<Vec<u8>>,
    size: Mutex<TerminalSize>,
    killed: AtomicBool,
}

/// Test-side handle of one fake terminal.
#[derive(Clone)]
pub struct FakeTerminal {
    inner: Arc<Inner>,
}

impl FakeTerminal {
    /// Makes the terminal print `bytes` as one read.
    pub fn emit(&self, bytes: &[u8]) {
        if let Some(output) = lock(&self.inner.output).as_ref() {
            let _ = output.send(bytes.to_vec());
        }
    }

    /// Makes the fake process exit by itself with `code`.
    pub fn exit(&self, code: u32) {
        if let Some(exit) = lock(&self.inner.exit).take() {
            let _ = exit.send(code);
        }
    }

    pub fn spec(&self) -> SpawnSpec {
        self.inner.spec.clone()
    }

    /// Everything written to the terminal so far.
    pub fn input(&self) -> Vec<u8> {
        lock(&self.inner.input).clone()
    }

    pub fn size(&self) -> TerminalSize {
        *lock(&self.inner.size)
    }

    pub fn was_killed(&self) -> bool {
        self.inner.killed.load(Ordering::SeqCst)
    }
}

impl Write for FakeTerminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        lock(&self.inner.input).extend_from_slice(buf);
        if self.inner.echo {
            self.emit(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl PtyControl for FakeTerminal {
    fn resize(&mut self, size: TerminalSize) -> Result<(), PtyError> {
        *lock(&self.inner.size) = size;
        Ok(())
    }

    fn kill_tree(&mut self) {
        if let Some(exit) = lock(&self.inner.exit).take() {
            self.inner.killed.store(true, Ordering::SeqCst);
            let _ = exit.send(KILLED_EXIT_CODE);
        }
    }

    fn release(&mut self) {
        lock(&self.inner.output).take();
    }

    fn tree_process_ids(&self) -> Vec<u32> {
        Vec::new()
    }
}

struct FakeReader {
    output: Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

impl Read for FakeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.pending.is_empty() {
            match self.output.recv() {
                Ok(bytes) => self.pending = bytes,
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

struct FakeWaiter(Receiver<u32>);

impl PtyWaiter for FakeWaiter {
    fn wait(self: Box<Self>) -> ExitStatus {
        ExitStatus {
            code: self.0.recv().unwrap_or(KILLED_EXIT_CODE),
        }
    }
}
