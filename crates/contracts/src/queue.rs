//! Lifecycle rules that depend on the current queue or task, beyond the wire shape.

use crate::{
    CommonIdV1, CommonQueueTaskStateV1, RecordQueueTaskV1, RecordQueueTaskV1Attempts,
    RecordTaskQueueV1, RecordTaskQueueV1State, RecordTaskQueueV1Version,
};

impl RecordTaskQueueV1 {
    pub fn new_stopped(id: CommonIdV1, owner_id: CommonIdV1) -> Self {
        Self {
            version: RecordTaskQueueV1Version,
            id,
            owner_id,
            state: RecordTaskQueueV1State::Stopped,
        }
    }

    /// The caller supplies the identity established by authentication.
    pub fn permits_state_change(
        &self,
        authenticated_owner: &CommonIdV1,
        next: RecordTaskQueueV1State,
    ) -> bool {
        self.owner_id == *authenticated_owner && self.state != next
    }
}

impl RecordQueueTaskV1 {
    /// Returns the attempt count after an allowed transition.
    pub fn next_attempt_count(
        &self,
        next: CommonQueueTaskStateV1,
    ) -> Option<RecordQueueTaskV1Attempts> {
        use CommonQueueTaskStateV1::{Cancelled, Done, Failed, Queued, Review, Running};

        let attempts = self.attempts.get();
        let limit = self.attempt_limit.get();
        if attempts > limit || (attempts == 0 && !matches!(self.state, Queued | Cancelled)) {
            return None;
        }

        match (self.state, next) {
            (Queued, Running) if attempts < limit => {
                RecordQueueTaskV1Attempts::new(attempts + 1).ok()
            }
            (Failed, Queued) if attempts < limit => Some(self.attempts),
            (Queued, Cancelled)
            | (Running, Review | Failed | Cancelled)
            | (Review, Done | Failed | Cancelled) => Some(self.attempts),
            _ => None,
        }
    }
}
