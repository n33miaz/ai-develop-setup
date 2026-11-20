//! External commitment to the database identity, log head and registered keys.
use crate::StorageError;

/// An opaque commitment in the version-1 storage format.
pub type Commitment = [u8; 105];

pub const COMMITMENT_VERSION: u8 = 1;
pub(crate) const GENESIS: std::ops::Range<usize> = 1..33;
pub(crate) const POSITION: std::ops::Range<usize> = 33..41;
pub(crate) const REGISTERED_KEYS: std::ops::RangeFrom<usize> = 73..;

/// Persistent storage for one audit log, independent of its database file.
pub trait AnchorStore: Send {
    fn read(&self) -> Result<Option<Commitment>, StorageError>;
    fn store(&self, commitment: &Commitment) -> Result<(), StorageError>;
}

/// The exact old and proposed commitments reviewed by the owner.
#[derive(Debug, PartialEq, Eq)]
pub struct ReanchorPlan {
    pub(crate) previous: Option<Commitment>,
    pub(crate) proposed: Commitment,
}

/// Changes the owner can inspect before confirming recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReanchorSummary {
    pub previous_position: Option<i64>,
    pub proposed_position: i64,
    /// Proposed position minus previous position. Negative means fewer rows.
    pub position_delta: Option<i128>,
    pub same_genesis: Option<bool>,
    pub same_registered_keys: Option<bool>,
}

impl ReanchorPlan {
    pub fn summary(&self) -> ReanchorSummary {
        fn position(commitment: &Commitment) -> i64 {
            i64::from_be_bytes(
                commitment[POSITION]
                    .try_into()
                    .expect("position is eight bytes"),
            )
        }

        let proposed_position = position(&self.proposed);
        let previous_position = self.previous.as_ref().map(position);
        ReanchorSummary {
            previous_position,
            proposed_position,
            position_delta: previous_position
                .map(|previous| i128::from(proposed_position) - i128::from(previous)),
            same_genesis: self
                .previous
                .as_ref()
                .map(|previous| previous[GENESIS] == self.proposed[GENESIS]),
            same_registered_keys: self
                .previous
                .as_ref()
                .map(|previous| previous[REGISTERED_KEYS] == self.proposed[REGISTERED_KEYS]),
        }
    }
}
