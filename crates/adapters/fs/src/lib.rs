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

pub use disk::{Midway, Mutation, Probe};
pub use executor::Executor;
pub use fence::FenceError;
pub use journal::{Journal, JournalError};
pub use limits::{DefinitiveDelete, Limits, LimitsError};
pub use manifest::{ITEMS_DIR, ItemKind, MANIFEST_NAME, MANIFEST_VERSION, Manifest, ManifestItem};
pub use reason::{Reason, Stop};
pub use report::{Completed, Effect, Failure, Laid, Report, Scaffold, Stage, Staged, Status, Step};
pub use survey::Facts;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_failures_are_standard_errors() {
        fn assert_error<T: std::error::Error>() {}

        assert_error::<Failure>();
        assert_error::<FenceError>();
        assert_error::<JournalError>();
        assert_error::<LimitsError>();
        assert_error::<Stop>();
        assert_error::<sys::Error>();
    }

    #[test]
    fn failures_keep_the_cause_and_compare_by_the_report_and_diagnostic() {
        use std::{error::Error, io};

        let error = sys::Error::from(io::Error::new(io::ErrorKind::PermissionDenied, "refused"));
        let stop = Stop::io("opening", "entry", error);
        let failure = Failure::before_anything(stop.clone());
        let with_message_only = Failure::before_anything(Stop::new(stop.reason, &stop.detail));
        assert_eq!(failure, with_message_only);
        assert_eq!(failure.to_string(), with_message_only.to_string());
        for error in [&stop as &dyn Error, failure.as_ref()] {
            let cause = error
                .source()
                .unwrap()
                .downcast_ref::<sys::Error>()
                .unwrap();
            let io_error = cause.source().unwrap().downcast_ref::<io::Error>().unwrap();
            assert_eq!(io_error.kind(), io::ErrorKind::PermissionDenied);
            assert_eq!(io_error.to_string(), "refused");
        }
    }

    #[test]
    fn configuration_and_manifest_errors_keep_parser_causes() {
        use std::{error::Error, io};

        let error = Limits::from_toml_str("max_depth = 'invalid'").unwrap_err();
        assert!(error.source().unwrap().is::<toml::de::Error>());
        let error = Limits::from_toml_str("private_dir = '..'").unwrap_err();
        assert!(error.source().unwrap().is::<io::Error>());
        let error = Manifest::from_bytes(b"{").unwrap_err();
        assert!(error.source().unwrap().is::<serde_json::Error>());
    }

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
