use crate::StorageError;
use serde::Deserialize;

const DEFAULTS: &str = include_str!("../../../defaults/storage.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub(crate) busy_timeout_ms: u32,
    pub(crate) inspection_max_bytes: usize,
    pub(crate) integrity_check_on_open: bool,
    pub(crate) max_event_bytes: usize,
    pub(crate) read_key_candidates: u32,
    pub(crate) read_page_limit: u32,
    pub(crate) read_page_max_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    busy_timeout_ms: Option<u32>,
    inspection_max_bytes: Option<usize>,
    integrity_check_on_open: Option<bool>,
    max_event_bytes: Option<usize>,
    read_key_candidates: Option<u32>,
    read_page_limit: Option<u32>,
    read_page_max_bytes: Option<usize>,
    encryption: Option<String>,
    cipher: Option<String>,
    journal_mode: Option<String>,
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
        Ok(Self {
            busy_timeout_ms,
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

    pub fn busy_timeout_ms(&self) -> u32 {
        self.busy_timeout_ms
    }
    pub fn integrity_check_on_open(&self) -> bool {
        self.integrity_check_on_open
    }
    pub fn max_event_bytes(&self) -> usize {
        self.max_event_bytes
    }
    pub fn read_page_limit(&self) -> u32 {
        self.read_page_limit
    }
    pub fn read_page_max_bytes(&self) -> usize {
        self.read_page_max_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        for text in [
            "read_key_candidates = 129",
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
