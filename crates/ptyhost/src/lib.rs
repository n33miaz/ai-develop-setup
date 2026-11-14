//! Terminal sessions behind a swappable backend, with sequence-numbered output and replay.

mod backend;
mod config;
mod fake;
mod host;
mod portable;
mod replay;
mod session;
mod types;
#[cfg(windows)]
pub mod win;

pub use backend::{PtyBackend, PtyControl, PtyParts, PtyWaiter};
pub use config::HostConfig;
pub use fake::{FakeBackend, FakeTerminal};
pub use host::PtyHost;
pub use portable::PortablePtyBackend;
pub use replay::ReplayRing;
pub use session::Subscription;
pub use types::{
    ExitStatus, OutputChunk, PtyError, ReplayFrom, Seq, SessionId, SpawnSpec, TerminalSize,
};
