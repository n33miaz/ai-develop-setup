//! The seam between sessions and whatever provides the terminal.

use std::io::{Read, Write};

use crate::types::{ExitStatus, PtyError, SpawnSpec, TerminalSize};

pub trait PtyBackend: Send + Sync {
    fn spawn(&self, spec: &SpawnSpec) -> Result<PtyParts, PtyError>;
}

/// One spawned terminal, split so output, input, control and exit can live on different threads.
pub struct PtyParts {
    /// Blocking output stream; end of file or an error means the terminal is gone.
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub control: Box<dyn PtyControl>,
    pub waiter: Box<dyn PtyWaiter>,
}

pub trait PtyControl: Send {
    fn resize(&mut self, size: TerminalSize) -> Result<(), PtyError>;
    /// Ends the root process and everything it started. Safe to call more than once.
    fn kill_tree(&mut self);
    /// Lets go of the terminal so the reader reaches its end. Safe to call more than once.
    fn release(&mut self);
    /// Processes of the session that are still alive (empty when the backend has none).
    fn tree_process_ids(&self) -> Vec<u32>;
}

pub trait PtyWaiter: Send {
    /// Blocks until the root process exits.
    fn wait(self: Box<Self>) -> ExitStatus;
}
