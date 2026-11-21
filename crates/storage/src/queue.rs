//! Queue streams: the events of each task queue in the anchored log, one stream per queue.
use crate::{EventLog, StorageConfig, StorageError, StoredEvent};
use ads_contracts::{CommonIdV1, EventEnvelopeV1Stream};
use rusqlite::OptionalExtension;
use serde_json::{Map, Value};

/// Stream family and event type family of the queue.
pub const QUEUE_FAMILY: &str = "queue";

/// Checks the binding of a queue event to its stream: a `queue.*` type only in a `queue/<id>`
/// stream and back, and `data.queue_id` equal to `<id>`. Other families pass untouched.
pub(crate) fn check_binding(
    type_family: &str,
    stream_family: &str,
    entity: &str,
    data: &Map<String, Value>,
) -> Result<(), StorageError> {
    let queue = |family: &str| family == QUEUE_FAMILY;
    if !queue(type_family) && !queue(stream_family) {
        return Ok(());
    }
    if type_family != stream_family {
        return Err(StorageError::InvalidEvent);
    }
    match data.get("queue_id") {
        Some(Value::String(id)) if id == entity && CommonIdV1::new(id.as_str()).is_ok() => Ok(()),
        _ => Err(StorageError::InvalidEvent),
    }
}

/// The only type the cap lets past: the stop version the core replays
/// (`QueueStreamEvent::from_envelope` in `ads-core`). Another version would make the replay refuse
/// the whole stream, so it stays behind the cap like any other event.
pub const QUEUE_STOPPED: &str = "urn:ads:event:queue.stopped:1";

/// Whether a queue event at `seq` may enter, checked inside the append transaction after the
/// compare and set. Up to `queue_max_events` every event enters. Above it only `QUEUE_STOPPED`
/// does, and only when the stream's previous event is not one: so at most one event lies past
/// the head the queue had when it reached the cap (a lowered cap included), and a full queue
/// can still be stopped after a restart. The bound is the storage's own, whatever the producer.
pub(crate) fn within_cap(
    tx: &rusqlite::Connection,
    kind: &str,
    stream: &str,
    seq: i64,
    config: &StorageConfig,
) -> Result<bool, StorageError> {
    let family = stream.split_once('/').map_or(stream, |(family, _)| family);
    if family != QUEUE_FAMILY || u64::try_from(seq).is_ok_and(|seq| seq <= config.queue_max_events)
    {
        return Ok(true);
    }
    if kind != QUEUE_STOPPED {
        return Ok(false);
    }
    let previous: Option<String> = tx
        .query_row(
            "SELECT type FROM event WHERE stream = ?1 AND seq = ?2",
            rusqlite::params![stream, seq - 1],
            |r| r.get(0),
        )
        .optional()?;
    Ok(previous.as_deref() != Some(QUEUE_STOPPED))
}

/// Stream of one queue.
pub fn queue_stream(queue_id: &CommonIdV1) -> EventEnvelopeV1Stream {
    EventEnvelopeV1Stream::new(format!("{QUEUE_FAMILY}/{}", queue_id.as_str()))
        .expect("a queue id fits a stream id")
}

impl EventLog {
    /// Every event of the queue, from `seq` 1 up to the head read when the call starts, in
    /// pages of `read_page_limit`. Each page is read and authenticated like `read_stream`, and
    /// the next page starts at the `seq` after the last one received, so a row missing between
    /// pages is `ChainBroken` too. It never refuses a stream for its length: lowering
    /// `queue_max_events` only stops new appends.
    pub fn read_queue(&self, queue_id: &CommonIdV1) -> Result<Vec<StoredEvent>, StorageError> {
        let stream = queue_stream(queue_id);
        let head = self.head(&stream)?;
        let mut events: Vec<StoredEvent> = Vec::new();
        let mut next = 1u64;
        while next <= head {
            let page = self.read_stream(&stream, next, u32::MAX)?;
            if page.is_empty() {
                return Err(StorageError::Corrupt);
            }
            for stored in page {
                if stored.event.seq.get() > head {
                    return Ok(events);
                }
                next = stored.event.seq.get() + 1;
                events.push(stored);
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            _ => unreachable!(),
        }
    }

    #[test]
    fn queue_events_bind_type_stream_and_queue_id() {
        let own = data(serde_json::json!({ "queue_id": "q1" }));
        assert_eq!(check_binding("queue", "queue", "q1", &own), Ok(()));
        assert_eq!(check_binding("task", "task", "t1", &own), Ok(()));
        assert_eq!(
            check_binding("task", "task", "t1", &data(serde_json::json!({}))),
            Ok(())
        );
        for (type_family, stream_family, entity, value) in [
            (
                "task",
                "queue",
                "q1",
                serde_json::json!({ "queue_id": "q1" }),
            ),
            (
                "queue",
                "task",
                "q1",
                serde_json::json!({ "queue_id": "q1" }),
            ),
            (
                "queue",
                "queue",
                "q2",
                serde_json::json!({ "queue_id": "q1" }),
            ),
            ("queue", "queue", "q1", serde_json::json!({})),
            (
                "queue",
                "queue",
                "q1",
                serde_json::json!({ "queue_id": null }),
            ),
            ("queue", "queue", "q1", serde_json::json!({ "queue_id": 1 })),
        ] {
            assert_eq!(
                check_binding(type_family, stream_family, entity, &data(value)),
                Err(StorageError::InvalidEvent),
                "{type_family} {stream_family} {entity}"
            );
        }
    }
}
