//! Adapter that runs gate-permitted file system actions inside a fence.
//!
//! The gate of `ads-core` decides with text; this crate is where the text meets the disk. The
//! only way in is the execution permit of the gate, and every path is opened name by name
//! from a fence given at construction, following no link; what is read, moved or removed is
//! what is open, never what a path happens to mean at that instant.

mod disk;
mod executor;
mod fence;
mod journal;
mod limits;
mod manifest;
mod prepare;
mod reason;
mod report;
mod survey;
mod sys;

pub use disk::{Mutation, Probe};
pub use executor::Executor;
pub use fence::FenceError;
pub use journal::{Journal, JournalError};
pub use limits::Limits;
pub use manifest::{ITEMS_DIR, ItemKind, MANIFEST_NAME, MANIFEST_VERSION, Manifest, ManifestItem};
pub use reason::{Reason, Stop};
pub use report::{Completed, Effect, Failure, Laid, Report, Scaffold, Stage, Staged, Status, Step};
pub use survey::Facts;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_its_own_contract_code() {
        let mut codes: Vec<&str> = Reason::ALL.iter().map(|reason| reason.as_str()).collect();
        for reason in Reason::ALL {
            assert_eq!(reason.error_code().as_str(), reason.as_str());
        }
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), Reason::ALL.len());
    }
}
