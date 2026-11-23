//! External commitment to the database identity, log head and registered keys.
use crate::StorageError;

/// An opaque commitment in the version-1 storage format.
pub type Commitment = [u8; 105];

pub const COMMITMENT_VERSION: u8 = 1;
pub(crate) const GENESIS: std::ops::Range<usize> = 1..33;
pub(crate) const POSITION: std::ops::Range<usize> = 33..41;
pub(crate) const REGISTERED_KEYS: std::ops::RangeFrom<usize> = 73..;

pub type DatabaseId = [u8; 32];

/// Persistent storage for one audit log, independent of its database file.
pub trait AnchorStore: Send {
    /// Whether a missing file or an empty unanchored log may be initialized.
    fn can_create(&self) -> Result<bool, StorageError>;
    fn read(&self, id: DatabaseId) -> Result<Option<Commitment>, StorageError>;
    fn store(&self, id: DatabaseId, commitment: &Commitment) -> Result<(), StorageError>;

    /// Commitment from the old reserved slot, if still eligible for adoption.
    fn legacy(&self) -> Result<Option<Commitment>, StorageError> {
        Ok(None)
    }

    fn recovery_anchor(&self, id: DatabaseId) -> Result<RecoveryAnchor, StorageError> {
        let (previous_id, commitment) = match self.read(id)? {
            Some(commitment) => (Some(id), Some(commitment)),
            None => {
                let legacy = self.legacy()?;
                (legacy.as_ref().and_then(genesis_id), legacy)
            }
        };
        Ok(RecoveryAnchor {
            previous_id,
            commitment,
            previous_location: None,
            proposed_location: None,
        })
    }

    fn recover(&self, plan: &ReanchorPlan) -> Result<(), StorageError> {
        self.store(plan.id, &plan.proposed)?;
        self.bind(plan.id)
    }

    /// Record the canonical identity after the scoped commitment is available.
    fn bind(&self, _id: DatabaseId) -> Result<(), StorageError> {
        Ok(())
    }
}

pub fn genesis_id(commitment: &Commitment) -> Option<DatabaseId> {
    let genesis: DatabaseId = commitment[GENESIS].try_into().expect("genesis is 32 bytes");
    (genesis != [0; 32]).then_some(genesis)
}

/// Vault state that must still match when the owner applies a recovery plan.
#[derive(Debug, PartialEq, Eq)]
pub struct RecoveryAnchor {
    pub previous_id: Option<DatabaseId>,
    pub commitment: Option<Commitment>,
    pub previous_location: Option<[u8; 32]>,
    pub proposed_location: Option<[u8; 32]>,
}

/// The exact old and proposed commitments reviewed by the owner.
#[derive(Debug, PartialEq, Eq)]
pub struct ReanchorPlan {
    pub(crate) id: DatabaseId,
    pub(crate) previous: RecoveryAnchor,
    pub(crate) proposed: Commitment,
}

/// Changes the owner can inspect before confirming recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReanchorSummary {
    pub previous_id: Option<DatabaseId>,
    pub proposed_id: DatabaseId,
    pub location_changed: Option<bool>,
    pub previous_position: Option<i64>,
    pub proposed_position: i64,
    /// Proposed position minus previous position. Negative means fewer rows.
    pub position_delta: Option<i128>,
    pub same_genesis: Option<bool>,
    pub same_registered_keys: Option<bool>,
}

impl ReanchorPlan {
    pub fn proposed_commitment(&self) -> &Commitment {
        &self.proposed
    }

    pub fn summary(&self) -> ReanchorSummary {
        fn position(commitment: &Commitment) -> i64 {
            i64::from_be_bytes(
                commitment[POSITION]
                    .try_into()
                    .expect("position is eight bytes"),
            )
        }

        let proposed_position = position(&self.proposed);
        let previous_position = self.previous.commitment.as_ref().map(position);
        ReanchorSummary {
            previous_id: self.previous.previous_id,
            proposed_id: self.id,
            location_changed: self
                .previous
                .previous_location
                .zip(self.previous.proposed_location)
                .map(|(old, new)| old != new),
            previous_position,
            proposed_position,
            position_delta: previous_position
                .map(|previous| i128::from(proposed_position) - i128::from(previous)),
            same_genesis: self
                .previous
                .commitment
                .as_ref()
                .and_then(|previous| genesis_id(previous).zip(genesis_id(&self.proposed)))
                .map(|(previous, proposed)| previous == proposed),
            same_registered_keys: self
                .previous
                .commitment
                .as_ref()
                .map(|previous| previous[REGISTERED_KEYS] == self.proposed[REGISTERED_KEYS]),
        }
    }
}
