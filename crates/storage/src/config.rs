use crate::StorageError;
use serde::Deserialize;

const DEFAULTS: &str = include_str!("../../../defaults/storage.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub(crate) busy_timeout_ms: u32,
    pub(crate) inspection_max_bytes: usize,
    pub(crate) integrity_check_on_open: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    busy_timeout_ms: Option<u32>,
    inspection_max_bytes: Option<usize>,
    integrity_check_on_open: Option<bool>,
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
        })
    }

    pub fn busy_timeout_ms(&self) -> u32 {
        self.busy_timeout_ms
    }
    pub fn integrity_check_on_open(&self) -> bool {
        self.integrity_check_on_open
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
        assert_eq!(
            over.integrity_check_on_open(),
            base.integrity_check_on_open()
        );
        for text in [
            "unknown = 1",
            "inspection_max_bytes = 511",
            "inspection_max_bytes = 1073741825",
            "busy_timeout_ms = 60001",
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
