//! A fake terminal whose blocking calls can be held at a gate, to see what the host does meanwhile.

use std::collections::HashSet;
use std::io::{self, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ads_ptyhost::{
    FakeBackend, PtyBackend, PtyControl, PtyError, PtyParts, SpawnSpec, TerminalSize,
};
use tokio::sync::mpsc;

/// A held call lets itself through after this long, so a host that stalls fails the test
/// instead of hanging it.
const HOLD_LIMIT: Duration = Duration::from_secs(10);

/// Program name that makes the spawn panic.
pub const PANIC_PROGRAM: &str = "panic";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    Spawn,
    Resize,
    Write,
    Kill,
}

#[derive(Default)]
struct State {
    held: HashSet<Op>,
    inside: HashSet<Op>,
    gave_up: bool,
}

pub struct Gates {
    state: Mutex<State>,
    changed: Condvar,
    entered: mpsc::UnboundedSender<Op>,
}

impl Gates {
    /// The gates and the stream that names each held call as it arrives.
    pub fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<Op>) {
        let (entered, arrivals) = mpsc::unbounded_channel();
        let gates = Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            entered,
        };
        (Arc::new(gates), arrivals)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// From now on a call of this kind stops at the gate until released.
    pub fn hold(&self, op: Op) {
        self.state().held.insert(op);
    }

    pub fn release(&self, op: Op) {
        self.state().held.remove(&op);
        self.changed.notify_all();
    }

    /// A call of this kind is stopped at the gate right now.
    pub fn is_inside(&self, op: Op) -> bool {
        self.state().inside.contains(&op)
    }

    /// Some held call waited the whole limit: nobody released it in time.
    pub fn gave_up(&self) -> bool {
        self.state().gave_up
    }

    fn pass(&self, op: Op) {
        let mut state = self.state();
        if !state.held.contains(&op) {
            return;
        }
        state.inside.insert(op);
        let _ = self.entered.send(op);
        let (mut state, wait) = self
            .changed
            .wait_timeout_while(state, HOLD_LIMIT, |state| state.held.contains(&op))
            .unwrap_or_else(PoisonError::into_inner);
        if wait.timed_out() {
            state.gave_up = true;
            state.held.remove(&op);
        }
        state.inside.remove(&op);
    }
}

pub struct GatedBackend {
    pub inner: Arc<FakeBackend>,
    pub gates: Arc<Gates>,
}

impl PtyBackend for GatedBackend {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtyParts, PtyError> {
        self.gates.pass(Op::Spawn);
        assert_ne!(spec.program, PANIC_PROGRAM, "spawn asked to panic");
        let parts = self.inner.spawn(spec)?;
        Ok(PtyParts {
            reader: parts.reader,
            writer: Box::new(GatedWriter {
                inner: parts.writer,
                gates: Arc::clone(&self.gates),
            }),
            control: Box::new(GatedControl {
                inner: parts.control,
                gates: Arc::clone(&self.gates),
            }),
            waiter: parts.waiter,
        })
    }
}

struct GatedWriter {
    inner: Box<dyn Write + Send>,
    gates: Arc<Gates>,
}

impl Write for GatedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.gates.pass(Op::Write);
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct GatedControl {
    inner: Box<dyn PtyControl>,
    gates: Arc<Gates>,
}

impl PtyControl for GatedControl {
    fn resize(&mut self, size: TerminalSize) -> Result<(), PtyError> {
        self.gates.pass(Op::Resize);
        self.inner.resize(size)
    }

    fn kill_tree(&mut self) {
        self.gates.pass(Op::Kill);
        self.inner.kill_tree();
    }

    fn release(&mut self) {
        self.inner.release();
    }

    fn tree_process_ids(&self) -> Vec<u32> {
        self.inner.tree_process_ids()
    }
}
