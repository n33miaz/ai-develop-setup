//! Streams de sessão limitados, sem persistência ou conteúdo em diagnósticos.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::watch;

use crate::api::{ApiError, checked};

pub fn new_id() -> Result<String, ApiError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| ApiError::Unavailable)?;
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::Unavailable)?
        .as_millis();
    bytes[..6].copy_from_slice(&ms.to_be_bytes()[10..]);
    let mut value = u128::from_be_bytes(bytes);
    let mut encoded = [b'0'; 26];
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    for byte in encoded.iter_mut().rev() {
        *byte = ALPHABET[(value & 31) as usize];
        value >>= 5;
    }
    String::from_utf8(encoded.to_vec()).map_err(|_| ApiError::Unavailable)
}

/// Fechamento de uma sessão cujo stream foi invalidado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closing {
    /// O daemon ainda tenta fechar a sessão no host; a entrega fica suspensa.
    Pending,
    /// As tentativas se esgotaram; falta um `session.close` confirmado.
    Failed,
    /// O host confirmou o fechamento.
    Closed,
}

/// O que uma assinatura faz com o estado atual do stream que assinou.
#[derive(Debug, PartialEq)]
pub enum Poll {
    Events(Vec<Value>),
    /// Fechamento automático em curso: nada a entregar por ora.
    Wait,
    /// Só esta assinatura termina, com este erro.
    End(ApiError),
    /// O cursor saiu da janela retida; a conexão fecha.
    Fatal(ApiError),
}

#[derive(Default)]
struct State {
    last: u64,
    events: VecDeque<Value>,
    invalid: Option<Closing>,
    /// O primeiro resultado final vence: é o que toda assinatura existente recebe, mesmo depois de a sessão mudar.
    outcome: Option<ApiError>,
    retired: bool,
    opened: bool,
}
impl State {
    fn finish(&mut self, outcome: ApiError) {
        self.outcome.get_or_insert(outcome);
    }
    /// Sai do log: a assinatura que ainda segura o stream recebe o fim, e os eventos deixam de ocupar memória.
    fn detach(&mut self) {
        self.events.clear();
        self.finish(ApiError::Replay);
    }
}

/// Uma encarnação de um stream. Cada assinatura guarda a sua; um id reaberto ganha outra.
#[derive(Default)]
pub struct Stream(Mutex<State>);
impl Stream {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Next step of a subscription to this incarnation after `after`.
    pub fn poll(&self, after: u64) -> Poll {
        let state = self.state();
        if let Some(outcome) = state.outcome {
            return Poll::End(outcome);
        }
        if state.invalid.is_some() {
            return Poll::Wait;
        }
        match since(&state, after) {
            Ok(events) => Poll::Events(events),
            Err(error) => Poll::Fatal(error),
        }
    }
    /// Whether events may still be delivered; checked again before each frame of a batch.
    pub fn deliverable(&self) -> bool {
        let state = self.state();
        state.outcome.is_none() && state.invalid.is_none()
    }
}
impl fmt::Debug for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Stream([redacted])")
    }
}

fn since(state: &State, after: u64) -> Result<Vec<Value>, ApiError> {
    let oldest = state
        .events
        .front()
        .and_then(|v| v["seq"].as_u64())
        .unwrap_or(state.last + 1);
    if after > state.last || (after != 0 && after + 1 < oldest) {
        return Err(ApiError::Replay);
    }
    Ok(state
        .events
        .iter()
        .filter(|e| e["seq"].as_u64().unwrap_or(0) > after)
        .cloned()
        .collect())
}

pub struct EventLog {
    streams: HashMap<String, Arc<Stream>>,
    capacity: usize,
    retired: VecDeque<String>,
    waiting: VecDeque<(String, Arc<Stream>)>,
    retired_capacity: usize,
    changed: watch::Sender<()>,
}

impl fmt::Debug for EventLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLog")
            .field("streams", &self.streams.len())
            .finish_non_exhaustive()
    }
}

impl EventLog {
    /// `retired_capacity` bounds both the tombstones of pruned ids and the streams subscribed before `session.open`.
    pub fn new(capacity: usize, retired_capacity: usize) -> Self {
        let (changed, _) = watch::channel(());
        Self {
            streams: HashMap::new(),
            capacity,
            retired: VecDeque::new(),
            waiting: VecDeque::new(),
            retired_capacity,
            changed,
        }
    }

    #[cfg(test)]
    pub(crate) fn exhaust_sequence(&mut self, session: &str) {
        let stream = Arc::new(Stream::default());
        stream.state().last = u64::MAX;
        self.streams.insert(format!("session/{session}"), stream);
    }
    fn evict(&mut self, name: &str) {
        if let Some(stream) = self.streams.remove(name) {
            let mut state = stream.state();
            if !state.opened && !state.retired {
                self.waiting.retain(|(waiting, _)| waiting != name);
            }
            state.detach();
        }
    }
    pub(crate) fn remove(&mut self, session: &str) {
        self.evict(&format!("session/{session}"));
        self.changed.send_replace(());
    }
    /// Replaces a pruned stream with a tombstone: its subscribers are told and the id cannot be reopened.
    pub fn retire(&mut self, session: &str) {
        let name = format!("session/{session}");
        let stream = self.streams.entry(name.clone()).or_default();
        let mut state = stream.state();
        if !state.opened && !state.retired {
            self.waiting.retain(|(waiting, _)| waiting != &name);
        }
        state.events.clear();
        state.finish(ApiError::Replay);
        if !state.retired {
            state.retired = true;
            self.retired.push_back(name);
        }
        drop(state);
        while self.retired.len() > self.retired_capacity {
            if let Some(expired) = self.retired.pop_front() {
                self.evict(&expired);
            }
        }
        self.changed.send_replace(());
    }
    pub fn is_retired(&self, stream: &str) -> bool {
        self.streams.get(stream).is_some_and(|s| s.state().retired)
    }
    pub fn invalidate(&mut self, session: &str, closing: Closing) {
        if let Some(stream) = self.streams.get(&format!("session/{session}")) {
            let mut state = stream.state();
            state.events.clear();
            state.invalid = Some(closing);
            match closing {
                Closing::Pending => (),
                Closing::Failed => state.finish(ApiError::ClosePending),
                Closing::Closed => state.finish(ApiError::Replay),
            }
        }
        self.changed.send_replace(());
    }
    pub fn watch(&self) -> watch::Receiver<()> {
        self.changed.subscribe()
    }

    pub fn append(
        &mut self,
        session: &str,
        kind: &str,
        data: Value,
        cause: Option<&str>,
    ) -> Result<(), ApiError> {
        let name = format!("session/{session}");
        let seq = self
            .streams
            .get(&name)
            .map_or(0, |s| s.state().last)
            .checked_add(1)
            .filter(|s| *s <= 9_007_199_254_740_991)
            .ok_or(ApiError::Capacity)?;
        let schema = format!("urn:ads:event:session.{kind}:1");
        let ts = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .map_err(|_| ApiError::Unavailable)?
            .format(&Rfc3339)
            .map_err(|_| ApiError::Unavailable)?;
        let event = checked(
            &schema,
            json!({
                "id": new_id()?, "type": schema, "ts": ts, "stream": name,
                "seq": seq, "actor": {"kind": "daemon", "id": "core"},
                "cause": cause, "data_class": "private", "data": data
            }),
        )?;
        // Um stream assinado antes do open é a mesma encarnação que a sessão passa a preencher.
        let stream = self.streams.entry(name.clone()).or_default();
        let mut state = stream.state();
        if !state.opened {
            self.waiting.retain(|(waiting, _)| waiting != &name);
            state.opened = true;
        }
        state.last = seq;
        state.events.push_back(event);
        while state.events.len() > self.capacity {
            state.events.pop_front();
        }
        drop(state);
        self.changed.send_replace(());
        Ok(())
    }

    /// Validates a subscription against the current state and returns the incarnation it follows.
    pub fn subscribe(&mut self, stream: &str, after: u64) -> Result<Arc<Stream>, ApiError> {
        let Some(entry) = self.streams.get(stream) else {
            // A interface pode assinar antes de session.open: a assinatura fica presa à encarnação que o open criar.
            if after != 0 {
                return Err(ApiError::Replay);
            }
            let entry = Arc::new(Stream::default());
            self.streams.insert(stream.to_owned(), entry.clone());
            self.waiting.push_back((stream.to_owned(), entry.clone()));
            if self.waiting.len() > self.retired_capacity
                && let Some((oldest, incarnation)) = self.waiting.pop_front()
                && self
                    .streams
                    .get(&oldest)
                    .is_some_and(|current| Arc::ptr_eq(current, &incarnation))
            {
                self.evict(&oldest);
                self.changed.send_replace(());
            }
            return Ok(entry);
        };
        {
            let state = entry.state();
            match state.invalid {
                Some(Closing::Failed) => return Err(ApiError::ClosePending),
                Some(_) => return Err(ApiError::Replay),
                None if state.retired => return Err(ApiError::Replay),
                None => (),
            }
            since(&state, after)?;
        }
        Ok(entry.clone())
    }

    /// Current events after `after`, for diagnostics and tests.
    pub fn after(&self, stream: &str, after: u64) -> Result<Vec<Value>, ApiError> {
        match self.streams.get(stream) {
            None if after == 0 => Ok(vec![]),
            None => Err(ApiError::Replay),
            Some(entry) => match entry.poll(after) {
                Poll::Events(events) => Ok(events),
                Poll::Wait => Err(ApiError::Replay),
                Poll::End(error) | Poll::Fatal(error) => Err(error),
            },
        }
    }
}
