//! Internal session types. What crosses the channel is in `wire`; the session name comes from there.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::wire::SessionId;

/// Position of an output chunk in a session. The first chunk is `Seq(1)`; numbers never repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seq(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChunk {
    pub seq: Seq,
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// What to run. The child sees only `env`: nothing is inherited from the host process.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub size: TerminalSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayFrom {
    /// Everything the ring still holds.
    Oldest,
    /// The chunk with this number and all after it.
    Seq(Seq),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: u32,
}

/// One open session as `session.list` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: SessionId,
    /// `None` while the root process is still running.
    pub exit: Option<ExitStatus>,
    pub oldest_seq: Seq,
    pub next_seq: Seq,
}

/// Why a subscription stopped delivering output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionEnd {
    /// The process exited and every piece of its output was delivered.
    Exited(ExitStatus),
    /// The session was closed while the subscription was open.
    SessionClosed,
    /// This subscriber fell too far behind; the rest is still in the replay ring.
    Lagged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyError {
    Config(String),
    Spawn(String),
    Io(String),
    UnknownSession(SessionId),
    SessionExists(SessionId),
    TooManySessions {
        limit: usize,
    },
    /// The requested chunk already left the ring; `oldest` is the first one still available.
    ReplayEvicted {
        requested: Seq,
        oldest: Seq,
    },
    /// The requested chunk does not exist yet; `next` is the number the next chunk will get.
    ReplayAhead {
        requested: Seq,
        next: Seq,
    },
    SessionEnded,
}

impl fmt::Display for PtyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "invalid configuration: {msg}"),
            Self::Spawn(msg) => write!(f, "spawn failed: {msg}"),
            Self::Io(msg) => write!(f, "terminal i/o failed: {msg}"),
            Self::UnknownSession(id) => write!(f, "unknown session {id}"),
            Self::SessionExists(id) => write!(f, "session {id} already exists"),
            Self::TooManySessions { limit } => write!(f, "session limit reached ({limit})"),
            Self::ReplayEvicted { requested, oldest } => write!(
                f,
                "seq {} left the replay ring; oldest available is {}",
                requested.0, oldest.0
            ),
            Self::ReplayAhead { requested, next } => {
                write!(f, "seq {} is ahead of the next seq {}", requested.0, next.0)
            }
            Self::SessionEnded => write!(f, "session already ended"),
        }
    }
}

impl std::error::Error for PtyError {}
