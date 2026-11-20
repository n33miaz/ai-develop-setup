//! External commitment to the database identity, log head and registered keys.
use crate::StorageError;

/// An opaque commitment in the version-1 storage format.
pub type Commitment = [u8; 105];

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
