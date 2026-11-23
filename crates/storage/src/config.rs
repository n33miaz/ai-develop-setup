use crate::StorageError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

const DEFAULTS: &str = include_str!("../../../defaults/storage.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    audit_database_path: Option<PathBuf>,
    pub(crate) busy_timeout_ms: u32,
    pub(crate) inspection_max_bytes: usize,
    pub(crate) integrity_check_on_open: bool,
    pub(crate) max_event_bytes: usize,
    pub(crate) queue_max_events: u64,
    pub(crate) read_key_candidates: u32,
    pub(crate) read_page_limit: u32,
    pub(crate) read_page_max_bytes: usize,
    pub(crate) verify_on_append: VerifyOnAppend,
}

/// When an anchored `append` authenticates the stored chain again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyOnAppend {
    /// Only after another connection committed (`PRAGMA data_version` moved).
    Changed,
    /// Before every append, at a cost proportional to the stored bytes.
    Always,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    audit_database_path: Option<PathBuf>,
    busy_timeout_ms: Option<u32>,
    inspection_max_bytes: Option<usize>,
    integrity_check_on_open: Option<bool>,
    max_event_bytes: Option<usize>,
    queue_max_events: Option<u64>,
    queue_retention: Option<String>,
    read_key_candidates: Option<u32>,
    read_page_limit: Option<u32>,
    read_page_max_bytes: Option<usize>,
    encryption: Option<String>,
    cipher: Option<String>,
    journal_mode: Option<String>,
    verify_on_append: Option<String>,
}

impl StorageConfig {
    pub fn defaults() -> Self {
        Self::from_toml_str("").expect("shipped storage defaults are valid")
    }

    pub fn from_toml_str(overrides: &str) -> Result<Self, StorageError> {
        let parse = |s| toml::from_str::<Layer>(s).map_err(|_| StorageError::Configuration);
        let base = parse(DEFAULTS)?;
        let over = parse(overrides)?;
        for (value, expected) in [
            (over.encryption.or(base.encryption), "column"),
            (over.cipher.or(base.cipher), "xchacha20poly1305"),
            (over.journal_mode.or(base.journal_mode), "wal"),
            (over.queue_retention.or(base.queue_retention), "all"),
        ] {
            if value.as_deref() != Some(expected) {
                return Err(StorageError::Configuration);
            }
        }
        let busy_timeout_ms = over
            .busy_timeout_ms
            .or(base.busy_timeout_ms)
            .filter(|n| *n <= 60_000)
            .ok_or(StorageError::Configuration)?;
        let max_event_bytes = over
            .max_event_bytes
            .or(base.max_event_bytes)
            .filter(|n| (64..=16_777_216).contains(n))
            .ok_or(StorageError::Configuration)?;
        // One event of the largest allowed size always fits in a page.
        let read_page_max_bytes = over
            .read_page_max_bytes
            .or(base.read_page_max_bytes)
            .filter(|n| (max_event_bytes..=268_435_456).contains(n))
            .ok_or(StorageError::Configuration)?;
        let verify_on_append = match over.verify_on_append.or(base.verify_on_append).as_deref() {
            Some("changed") => VerifyOnAppend::Changed,
            Some("always") => VerifyOnAppend::Always,
            _ => return Err(StorageError::Configuration),
        };
        let audit_database_path = over.audit_database_path.or(base.audit_database_path);
        if audit_database_path.as_ref().is_some_and(|path| {
            !path.is_absolute()
                || path.file_name().is_none()
                || path
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
        }) {
            return Err(StorageError::Configuration);
        }
        Ok(Self {
            audit_database_path,
            busy_timeout_ms,
            verify_on_append,
            inspection_max_bytes: over
                .inspection_max_bytes
                .or(base.inspection_max_bytes)
                .filter(|n| (512..=1_073_741_824).contains(n))
                .ok_or(StorageError::Configuration)?,
            integrity_check_on_open: over
                .integrity_check_on_open
                .or(base.integrity_check_on_open)
                .ok_or(StorageError::Configuration)?,
            max_event_bytes,
            read_page_max_bytes,
            queue_max_events: over
                .queue_max_events
                .or(base.queue_max_events)
                .filter(|n| (1..=10_000_000).contains(n))
                .ok_or(StorageError::Configuration)?,
            read_key_candidates: over
                .read_key_candidates
                .or(base.read_key_candidates)
                .filter(|n| *n <= 128)
                .ok_or(StorageError::Configuration)?,
            read_page_limit: over
                .read_page_limit
                .or(base.read_page_limit)
                .filter(|n| (1..=10_000).contains(n))
                .ok_or(StorageError::Configuration)?,
        })
    }

    pub fn audit_database_path(&self) -> Option<&Path> {
        self.audit_database_path.as_deref()
    }

    pub fn busy_timeout_ms(&self) -> u32 {
        self.busy_timeout_ms
    }
    pub fn integrity_check_on_open(&self) -> bool {
        self.integrity_check_on_open
    }
    pub fn max_event_bytes(&self) -> usize {
        self.max_event_bytes
    }
    pub fn queue_max_events(&self) -> u64 {
        self.queue_max_events
    }
    pub fn read_page_limit(&self) -> u32 {
        self.read_page_limit
    }
    pub fn read_page_max_bytes(&self) -> usize {
        self.read_page_max_bytes
    }
    pub fn verify_on_append(&self) -> VerifyOnAppend {
        self.verify_on_append
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_database_path_is_absent_by_default() {
        assert_eq!(StorageConfig::defaults().audit_database_path(), None);
    }

    #[test]
    fn audit_database_path_accepts_absolute_path_with_file_name() {
        #[cfg(windows)]
        let path = r"Z:\fictional-storage\audit.sqlite";
        #[cfg(not(windows))]
        let path = "/fictional-storage/audit.sqlite";

        let config =
            StorageConfig::from_toml_str(&format!("audit_database_path = '{path}'")).unwrap();
        assert_eq!(config.audit_database_path(), Some(Path::new(path)));
    }

    #[test]
    fn audit_database_path_rejects_relative_path() {
        assert_eq!(
            StorageConfig::from_toml_str("audit_database_path = 'audit.sqlite'"),
            Err(StorageError::Configuration)
        );
    }

    #[cfg(windows)]
    #[test]
    fn audit_database_path_rejects_drive_relative_path() {
        assert_eq!(
            StorageConfig::from_toml_str("audit_database_path = 'Z:audit.sqlite'"),
            Err(StorageError::Configuration)
        );
    }

    #[cfg(windows)]
    #[test]
    fn audit_database_path_rejects_rooted_path_without_drive() {
        assert_eq!(
            StorageConfig::from_toml_str(r"audit_database_path = '\audit.sqlite'"),
            Err(StorageError::Configuration)
        );
    }

    #[test]
    fn audit_database_path_rejects_root() {
        #[cfg(windows)]
        let path = r"Z:\";
        #[cfg(not(windows))]
        let path = "/";

        assert_eq!(
            StorageConfig::from_toml_str(&format!("audit_database_path = '{path}'")),
            Err(StorageError::Configuration)
        );
    }

    #[test]
    fn audit_database_path_rejects_parent_component() {
        #[cfg(windows)]
        let path = r"Z:\fictional-storage\..\audit.sqlite";
        #[cfg(not(windows))]
        let path = "/fictional-storage/../audit.sqlite";

        assert_eq!(
            StorageConfig::from_toml_str(&format!("audit_database_path = '{path}'")),
            Err(StorageError::Configuration)
        );
    }

    #[test]
    fn layers_validate_without_echoing_input() {
        let base = StorageConfig::defaults();
        let over = StorageConfig::from_toml_str("busy_timeout_ms = 17").unwrap();
        assert_eq!(over.busy_timeout_ms(), 17);
        assert_eq!(base.max_event_bytes(), 1_048_576);
        assert_eq!(base.read_page_limit(), 500);
        assert_eq!(base.read_page_max_bytes(), 8_388_608);
        let paged = StorageConfig::from_toml_str(
            "max_event_bytes = 64
read_page_max_bytes = 64",
        )
        .unwrap();
        assert_eq!(paged.read_page_max_bytes(), 64);
        let maximum_sizes = StorageConfig::from_toml_str(
            "max_event_bytes = 16_777_216
read_page_max_bytes = 268_435_456",
        )
        .unwrap();
        assert_eq!(maximum_sizes.max_event_bytes(), 16_777_216);
        assert_eq!(maximum_sizes.read_page_max_bytes(), 268_435_456);
        let over_log = StorageConfig::from_toml_str(
            "max_event_bytes = 64
read_page_limit = 1",
        )
        .unwrap();
        assert_eq!(
            (over_log.max_event_bytes(), over_log.read_page_limit()),
            (64, 1)
        );
        assert_eq!(
            over.integrity_check_on_open(),
            base.integrity_check_on_open()
        );
        assert_eq!(base.read_key_candidates, 8);
        assert_eq!(
            StorageConfig::from_toml_str("read_key_candidates = 0")
                .unwrap()
                .read_key_candidates,
            0
        );
        assert_eq!(base.queue_max_events(), 100_000);
        assert_eq!(
            StorageConfig::from_toml_str("queue_max_events = 1")
                .unwrap()
                .queue_max_events(),
            1
        );
        assert_eq!(
            StorageConfig::from_toml_str("queue_max_events = 10000000")
                .unwrap()
                .queue_max_events(),
            10_000_000
        );
        assert_eq!(base.verify_on_append(), VerifyOnAppend::Changed);
        assert_eq!(
            StorageConfig::from_toml_str("verify_on_append = 'always'")
                .unwrap()
                .verify_on_append(),
            VerifyOnAppend::Always
        );
        for text in [
            "verify_on_append = 'never'",
            "verify_on_append = 'Always'",
            "verify_on_append = 1",
            "read_key_candidates = 129",
            "queue_max_events = 0",
            "queue_max_events = 10000001",
            "queue_retention = 'days'",
            "unknown = 1",
            "inspection_max_bytes = 511",
            "inspection_max_bytes = 1073741825",
            "busy_timeout_ms = 60001",
            "max_event_bytes = 63",
            "max_event_bytes = 16777217",
            "read_page_limit = 0",
            "read_page_limit = 10001",
            "read_page_max_bytes = 1048575",
            "read_page_max_bytes = 268435457",
            "max_event_bytes = 16777216",
            "synchronous = 'normal'",
            "encryption = 'none'",
            "cipher = 'other'",
            "journal_mode = 'delete'",
        ] {
            assert_eq!(
                StorageConfig::from_toml_str(text),
                Err(StorageError::Configuration)
            );
        }
    }
}
