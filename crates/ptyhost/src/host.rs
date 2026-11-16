//! The set of open sessions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::backend::PtyBackend;
use crate::config::HostConfig;
use crate::session::{Session, Subscription, lock};
use crate::types::{ExitStatus, PtyError, ReplayFrom, SessionInfo, SpawnSpec, TerminalSize};
use crate::wire::SessionId;

enum Slot {
    /// Name and place taken while the terminal is being created, outside the lock.
    Opening,
    Open(Arc<Session>),
}

type Sessions = Mutex<HashMap<SessionId, Slot>>;

/// Gives the name and the place back if the session never gets to open, panic included.
struct Reservation<'a> {
    sessions: &'a Sessions,
    id: &'a SessionId,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut sessions = lock(self.sessions);
        if matches!(sessions.get(self.id), Some(Slot::Opening)) {
            sessions.remove(self.id);
        }
    }
}

/// The lock over the set is held only to look at it or change it: whatever may wait on a
/// terminal (spawn, write, resize, kill) runs with the lock released.
pub struct PtyHost {
    backend: Arc<dyn PtyBackend>,
    config: HostConfig,
    sessions: Sessions,
}

impl PtyHost {
    pub fn new(backend: Arc<dyn PtyBackend>, config: HostConfig) -> Result<Self, PtyError> {
        config.validate()?;
        Ok(Self {
            backend,
            config,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    pub fn config(&self) -> &HostConfig {
        &self.config
    }

    /// Starts a session under the name the client chose. Blocks while the terminal is created;
    /// until then the name is taken and the session is not there to be used.
    pub fn open(&self, id: &SessionId, spec: &SpawnSpec) -> Result<(), PtyError> {
        {
            let mut sessions = lock(&self.sessions);
            if sessions.contains_key(id) {
                return Err(PtyError::SessionExists(id.clone()));
            }
            if sessions.len() >= self.config.max_sessions {
                return Err(PtyError::TooManySessions {
                    limit: self.config.max_sessions,
                });
            }
            sessions.insert(id.clone(), Slot::Opening);
        }
        let reservation = Reservation {
            sessions: &self.sessions,
            id,
        };
        let session = Session::start(self.backend.spawn(spec)?, &self.config)?;
        let mut sessions = lock(&self.sessions);
        sessions.insert(id.clone(), Slot::Open(Arc::new(session)));
        // Published under the same lock: from here on a reservation of this name is someone else's.
        std::mem::forget(reservation);
        Ok(())
    }

    /// Output from the requested point on, followed by live output.
    pub fn subscribe(&self, id: &SessionId, from: ReplayFrom) -> Result<Subscription, PtyError> {
        self.session(id)?.subscribe(from)
    }

    pub fn write(&self, id: &SessionId, bytes: &[u8]) -> Result<(), PtyError> {
        self.session(id)?.write(bytes)
    }

    pub fn resize(&self, id: &SessionId, size: TerminalSize) -> Result<(), PtyError> {
        self.session(id)?.resize(size)
    }

    /// `None` while the root process is still running.
    pub fn exit_status(&self, id: &SessionId) -> Result<Option<ExitStatus>, PtyError> {
        Ok(self.session(id)?.exit_status())
    }

    pub async fn wait_exit(&self, id: &SessionId) -> Result<ExitStatus, PtyError> {
        self.session(id)?.wait_exit().await
    }

    /// Processes of the session that are still alive, as far as the backend can tell.
    pub fn tree_process_ids(&self, id: &SessionId) -> Result<Vec<u32>, PtyError> {
        Ok(self.session(id)?.tree_process_ids())
    }

    /// Subscriptions of the session that still have a live queue.
    pub fn subscriber_count(&self, id: &SessionId) -> Result<usize, PtyError> {
        Ok(self.session(id)?.subscriber_count())
    }

    pub fn session_ids(&self) -> Vec<SessionId> {
        let mut ids: Vec<_> = self.open_sessions().into_iter().map(|(id, _)| id).collect();
        ids.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
        ids
    }

    /// Every open session, ordered by name.
    pub fn sessions(&self) -> Vec<SessionInfo> {
        let mut sessions: Vec<_> = self
            .open_sessions()
            .into_iter()
            .map(|(id, session)| {
                let (oldest_seq, next_seq) = session.replay_bounds();
                SessionInfo {
                    id,
                    exit: session.exit_status(),
                    oldest_seq,
                    next_seq,
                }
            })
            .collect();
        sessions.sort_unstable_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        sessions
    }

    /// Ends the session: kills its process tree, forgets its replay and frees its slot.
    pub async fn close(&self, id: &SessionId) -> Result<ExitStatus, PtyError> {
        let session = {
            let mut sessions = lock(&self.sessions);
            // A session still being opened is not there to be closed: its place stays reserved.
            match sessions.get(id) {
                Some(Slot::Open(_)) => sessions.remove(id),
                _ => None,
            }
        };
        let Some(Slot::Open(session)) = session else {
            return Err(PtyError::UnknownSession(id.clone()));
        };
        // The session goes with the closure, so its last reference is dropped off the async threads.
        tokio::task::spawn_blocking(move || session.close())
            .await
            .map_err(|err| PtyError::Io(err.to_string()))?
    }

    /// The sessions taken out from under the lock, so reading them never holds it.
    fn open_sessions(&self) -> Vec<(SessionId, Arc<Session>)> {
        lock(&self.sessions)
            .iter()
            .filter_map(|(id, slot)| match slot {
                Slot::Open(session) => Some((id.clone(), Arc::clone(session))),
                Slot::Opening => None,
            })
            .collect()
    }

    fn session(&self, id: &SessionId) -> Result<Arc<Session>, PtyError> {
        match lock(&self.sessions).get(id) {
            Some(Slot::Open(session)) => Ok(Arc::clone(session)),
            _ => Err(PtyError::UnknownSession(id.clone())),
        }
    }
}
