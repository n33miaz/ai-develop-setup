//! Terminal sessions behind a swappable backend, with sequence-numbered output and replay,
//! served to the core over a local channel that speaks the `pty-host/1` contract.

mod backend;
pub mod channel;
mod config;
mod fake;
mod host;
mod portable;
mod replay;
pub mod server;
mod session;
mod types;
#[cfg(windows)]
pub mod win;
pub mod wire;

pub use backend::{PtyBackend, PtyControl, PtyParts, PtyWaiter};
pub use config::HostConfig;
pub use fake::{FakeBackend, FakeTerminal};
pub use host::PtyHost;
pub use portable::PortablePtyBackend;
pub use replay::ReplayRing;
pub use session::Subscription;
pub use types::{
    ExitStatus, OutputChunk, PtyError, ReplayFrom, Seq, SessionInfo, SpawnSpec, SubscriptionEnd,
    TerminalSize,
};
pub use wire::SessionId;
