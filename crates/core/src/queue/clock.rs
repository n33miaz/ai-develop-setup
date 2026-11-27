//! Owner anchor of the admission clock.

use ads_contracts::{
    CommonIdV1, CommonTimestampV1, EventQueueClockReanchoredV1, EventQueueClockReanchoredV1Data,
    EventQueueClockReanchoredV1DataVia, RecordTaskQueueV1,
};

use super::{AuthenticatedDevice, Channel, QueueRefusal};

/// Owner action that re-anchors the admission clock of a queue after the core clock ran ahead.
/// Earlier admissions of the queue, up to `through_seq`, stamped after `at` and still ahead of
/// the core clock count as admitted at `at`: they keep their origin reserved and fill the window
/// that follows the anchor. The admission refuses an anchor that covers the candidate or a later
/// `seq`, or whose `at` is ahead of the core clock.
///
/// Built from an authenticated owner action or restored by replay after validating the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockAnchor {
    pub(super) queue_id: CommonIdV1,
    pub(super) owner_id: CommonIdV1,
    device_id: CommonIdV1,
    via: Channel,
    pub(super) at: CommonTimestampV1,
    pub(super) through_seq: u64,
}

impl ClockAnchor {
    pub(super) fn from_event(event: &EventQueueClockReanchoredV1) -> Self {
        Self {
            queue_id: event.data.queue_id.clone(),
            owner_id: event.data.owner_id.clone(),
            device_id: event.actor.id.clone(),
            via: match event.data.via {
                EventQueueClockReanchoredV1DataVia::LocalUi => Channel::LocalUi,
                EventQueueClockReanchoredV1DataVia::Tray => Channel::Tray,
                EventQueueClockReanchoredV1DataVia::RemoteDevice => Channel::RemoteDevice,
            },
            at: event.ts.clone(),
            through_seq: event.seq.get() - 1,
        }
    }

    /// Payload to append at `through_seq + 1`, with `at` as the envelope timestamp.
    pub fn event_data(&self) -> EventQueueClockReanchoredV1Data {
        EventQueueClockReanchoredV1Data {
            queue_id: self.queue_id.clone(),
            owner_id: self.owner_id.clone(),
            via: match self.via {
                Channel::LocalUi => EventQueueClockReanchoredV1DataVia::LocalUi,
                Channel::Tray => EventQueueClockReanchoredV1DataVia::Tray,
                Channel::RemoteDevice => EventQueueClockReanchoredV1DataVia::RemoteDevice,
            },
        }
    }

    pub fn queue_id(&self) -> &CommonIdV1 {
        &self.queue_id
    }

    pub fn owner_id(&self) -> &CommonIdV1 {
        &self.owner_id
    }

    pub fn device_id(&self) -> &CommonIdV1 {
        &self.device_id
    }

    pub fn via(&self) -> Channel {
        self.via
    }

    /// Core clock when the owner acted.
    pub fn at(&self) -> &CommonTimestampV1 {
        &self.at
    }

    /// Last `seq` of the queue stream when the owner acted.
    pub fn through_seq(&self) -> u64 {
        self.through_seq
    }
}

/// Builds the anchor from the owner action. `now` is the core clock when the owner acts, with
/// the same contract as `created.ts`; `through_seq` is the `seq` the owner acted on, and
/// `stream_last_seq` the last `seq` of the queue stream read from storage, which must be equal.
/// The producer conditionally appends the event at the next sequence, using the owner device as
/// actor and `now` as `ts`. The anchor takes effect only after the append succeeds.
pub fn reanchor_admission_clock(
    queue: &RecordTaskQueueV1,
    device: Option<&AuthenticatedDevice>,
    now: &CommonTimestampV1,
    through_seq: u64,
    stream_last_seq: u64,
) -> Result<ClockAnchor, QueueRefusal> {
    let device = device.ok_or(QueueRefusal::Unauthenticated)?;
    if device.owner_id != queue.owner_id {
        return Err(QueueRefusal::NotOwner);
    }
    if through_seq != stream_last_seq {
        return Err(QueueRefusal::StaleSeq);
    }
    Ok(ClockAnchor {
        queue_id: queue.id.clone(),
        owner_id: device.owner_id.clone(),
        device_id: device.device_id.clone(),
        via: device.via,
        at: now.clone(),
        through_seq,
    })
}
