//! Local SQLite storage, column encryption and chained audit.

pub mod cipher;

pub mod config;
pub mod error;
mod migrate;
pub mod open;
pub use config::StorageConfig;
pub use error::StorageError;
pub use open::AuditStore;

/// Package name as declared in the manifest.
pub const fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(crate_name(), "ads-storage");
    }
}
