//! Streams de sessão limitados, sem persistência ou conteúdo em diagnósticos.

use std::collections::{HashMap, VecDeque};
use std::fmt;
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

/// O que uma assinatura existente faz com o estado atual do seu stream.
#[derive(Debug, PartialEq)]
pub enum Poll {
    /// Eventos novos e a geração do stream, que a assinatura passa a seguir.
    Events(Vec<Value>, Option<u64>),
    /// Nada a entregar por ora.
    Wait,
    /// Só esta assinatura termina, com este erro.
    End(ApiError),
    /// O cursor saiu da janela retida; a conexão fecha.
    Fatal(ApiError),
}

#[derive(Default)]
struct Stream {
    generation: u64,
    last: u64,
    invalid: Option<Closing>,
    retired: bool,
    events: VecDeque<Value>,
}

pub struct EventLog {
    streams: HashMap<String, Stream>,
    capacity: usize,
    retired: VecDeque<String>,
    retired_capacity: usize,
    generations: u64,
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
    /// `retired_capacity` bounds the tombstones that keep pruned session ids from being reused.
    pub fn new(capacity: usize, retired_capacity: usize) -> Self {
        let (changed, _) = watch::channel(());
        Self {
            streams: HashMap::new(),
            capacity,
            retired: VecDeque::new(),
            retired_capacity,
            generations: 0,
            changed,
        }
    }

    #[cfg(test)]
    pub(crate) fn exhaust_sequence(&mut self, session: &str) {
        self.stream(format!("session/{session}")).last = u64::MAX;
    }
    fn stream(&mut self, name: String) -> &mut Stream {
        let generations = &mut self.generations;
        self.streams.entry(name).or_insert_with(|| {
            *generations += 1;
            Stream {
                generation: *generations,
                ..Stream::default()
            }
        })
    }
    pub(crate) fn remove(&mut self, session: &str) {
        self.streams.remove(&format!("session/{session}"));
        self.changed.send_replace(());
    }
    /// Replaces a pruned stream with a tombstone: its subscribers are told and the id cannot be reopened.
    pub fn retire(&mut self, session: &str) {
        let name = format!("session/{session}");
        let stream = self.stream(name.clone());
        stream.events.clear();
        if !stream.retired {
            stream.retired = true;
            self.retired.push_back(name);
        }
        while self.retired.len() > self.retired_capacity {
            if let Some(expired) = self.retired.pop_front() {
                self.streams.remove(&expired);
            }
        }
        self.changed.send_replace(());
    }
    pub fn is_retired(&self, stream: &str) -> bool {
        self.streams.get(stream).is_some_and(|s| s.retired)
    }
    /// Generation of the current incarnation of a stream; a reused id always gets a new one.
    pub fn generation(&self, stream: &str) -> Option<u64> {
        self.streams.get(stream).map(|s| s.generation)
    }
    pub fn invalidate(&mut self, session: &str, closing: Closing) {
        if let Some(stream) = self.streams.get_mut(&format!("session/{session}")) {
            stream.events.clear();
            stream.invalid = Some(closing);
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
        let stream = format!("session/{session}");
        let seq = self
            .streams
            .get(&stream)
            .map_or(0, |s| s.last)
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
                "id": new_id()?, "type": schema, "ts": ts, "stream": stream,
                "seq": seq, "actor": {"kind": "daemon", "id": "core"},
                "cause": cause, "data_class": "private", "data": data
            }),
        )?;
        let capacity = self.capacity;
        let entry = self.stream(stream);
        entry.last = seq;
        entry.events.push_back(event);
        while entry.events.len() > capacity {
            entry.events.pop_front();
        }
        self.changed.send_replace(());
        Ok(())
    }

    /// Validates a new subscription; an invalidated or pruned stream refuses it.
    pub fn after(&self, stream: &str, after: u64) -> Result<Vec<Value>, ApiError> {
        let Some(entry) = self.streams.get(stream) else {
            // A interface pode assinar antes de session.open.
            return if after == 0 {
                Ok(vec![])
            } else {
                Err(ApiError::Replay)
            };
        };
        match entry.invalid {
            Some(Closing::Failed) => return Err(ApiError::ClosePending),
            Some(_) => return Err(ApiError::Replay),
            None if entry.retired => return Err(ApiError::Replay),
            None => (),
        }
        Self::since(entry, after)
    }

    fn since(entry: &Stream, after: u64) -> Result<Vec<Value>, ApiError> {
        let oldest = entry
            .events
            .front()
            .and_then(|v| v["seq"].as_u64())
            .unwrap_or(entry.last + 1);
        if after > entry.last || (after != 0 && after + 1 < oldest) {
            return Err(ApiError::Replay);
        }
        Ok(entry
            .events
            .iter()
            .filter(|e| e["seq"].as_u64().unwrap_or(0) > after)
            .cloned()
            .collect())
    }

    /// Next step of a subscription bound to `generation` (absent when it began before the stream existed).
    pub fn poll(&self, stream: &str, after: u64, generation: Option<u64>) -> Poll {
        let Some(entry) = self.streams.get(stream) else {
            // Sem a lápide, a geração guardada ainda prova que o stream assinado acabou.
            return match (generation, after) {
                (Some(_), _) => Poll::End(ApiError::Replay),
                (None, 0) => Poll::Events(vec![], None),
                (None, _) => Poll::Fatal(ApiError::Replay),
            };
        };
        if entry.retired || generation.is_some_and(|g| g != entry.generation) {
            return Poll::End(ApiError::Replay);
        }
        match entry.invalid {
            Some(Closing::Pending) => Poll::Wait,
            Some(Closing::Failed) => Poll::End(ApiError::ClosePending),
            Some(Closing::Closed) => Poll::End(ApiError::Replay),
            None => match Self::since(entry, after) {
                Ok(events) => Poll::Events(events, Some(entry.generation)),
                Err(error) => Poll::Fatal(error),
            },
        }
    }
}
