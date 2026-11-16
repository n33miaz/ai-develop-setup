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

#[derive(Default)]
struct Stream {
    last: u64,
    events: VecDeque<Value>,
}

pub struct EventLog {
    streams: HashMap<String, Stream>,
    capacity: usize,
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
    pub fn new(capacity: usize) -> Self {
        let (changed, _) = watch::channel(());
        Self {
            streams: HashMap::new(),
            capacity,
            changed,
        }
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
        let entry = self.streams.entry(stream).or_default();
        entry.last = seq;
        entry.events.push_back(event);
        while entry.events.len() > self.capacity {
            entry.events.pop_front();
        }
        self.changed.send_replace(());
        Ok(())
    }

    pub fn after(&self, stream: &str, after: u64) -> Result<Vec<Value>, ApiError> {
        let Some(entry) = self.streams.get(stream) else {
            // A interface pode assinar antes de session.open.
            return if after == 0 {
                Ok(vec![])
            } else {
                Err(ApiError::Replay)
            };
        };
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
}
