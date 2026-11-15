//! The set of open sessions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::backend::PtyBackend;
use crate::config::HostConfig;
use crate::session::{Session, Subscription, lock};
use crate::types::{ExitStatus, PtyError, ReplayFrom, SessionInfo, SpawnSpec, TerminalSize};
use crate::wire::SessionId;

pub struct PtyHost {
    backend: Arc<dyn PtyBackend>,
    config: HostConfig,
    sessions: Mutex<HashMap<SessionId, Arc<Session>>>,
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

    /// Starts a session under the name the client chose.
    pub fn open(&self, id: &SessionId, spec: &SpawnSpec) -> Result<(), PtyError> {
        let mut sessions = lock(&self.sessions);
        if sessions.contains_key(id) {
            return Err(PtyError::SessionExists(id.clone()));
        }
        if sessions.len() >= self.config.max_sessions {
            return Err(PtyError::TooManySessions {
                limit: self.config.max_sessions,
            });
        }
        let session = Session::start(self.backend.spawn(spec)?, &self.config)?;
        sessions.insert(id.clone(), Arc::new(session));
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

    pub fn session_ids(&self) -> Vec<SessionId> {
        let mut ids: Vec<_> = lock(&self.sessions).keys().cloned().collect();
        ids.sort_unstable();
        ids
    }

    /// Every open session, ordered by name.
    pub fn sessions(&self) -> Vec<SessionInfo> {
        let mut sessions: Vec<_> = lock(&self.sessions)
            .iter()
            .map(|(id, session)| {
                let (oldest_seq, next_seq) = session.replay_bounds();
                SessionInfo {
                    id: id.clone(),
                    exit: session.exit_status(),
                    oldest_seq,
                    next_seq,
                }
            })
            .collect();
        sessions.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        sessions
    }

    /// Ends the session: kills its process tree, forgets its replay and frees its slot.
    pub async fn close(&self, id: &SessionId) -> Result<ExitStatus, PtyError> {
        let session = lock(&self.sessions)
            .remove(id)
            .ok_or_else(|| PtyError::UnknownSession(id.clone()))?;
        session.close().await
    }

    fn session(&self, id: &SessionId) -> Result<Arc<Session>, PtyError> {
        lock(&self.sessions)
            .get(id)
            .cloned()
            .ok_or_else(|| PtyError::UnknownSession(id.clone()))
    }
}
