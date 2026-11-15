//! Parameters of the executor. The shipped values live only in `defaults/fs-executor.toml`.

use serde::Deserialize;

const DEFAULTS_TOML: &str = include_str!("../../../../defaults/fs-executor.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many folders deep a tree may go before the executor refuses it.
    pub max_depth: u32,
    /// The largest manifest the executor reads, in bytes.
    pub max_manifest_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    max_depth: Option<u32>,
    max_manifest_bytes: Option<u64>,
}

impl Limits {
    pub fn defaults() -> Self {
        Self::from_toml_str("").expect("the shipped defaults are complete")
    }

    /// The shipped defaults with the keys of `overrides` applied.
    pub fn from_toml_str(overrides: &str) -> Result<Self, String> {
        let parse = |text: &str| toml::from_str::<Layer>(text).map_err(|error| error.to_string());
        let base = parse(DEFAULTS_TOML)?;
        let over = parse(overrides)?;
        let missing = |key: &str| format!("missing key `{key}`");
        Ok(Self {
            max_depth: over
                .max_depth
                .or(base.max_depth)
                .ok_or_else(|| missing("max_depth"))?,
            max_manifest_bytes: over
                .max_manifest_bytes
                .or(base.max_manifest_bytes)
                .ok_or_else(|| missing("max_manifest_bytes"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_load_and_a_layer_replaces_one_key() {
        let defaults = Limits::defaults();
        assert!(defaults.max_depth > 0 && defaults.max_manifest_bytes > 0);
        let layered = Limits::from_toml_str("max_depth = 3").unwrap();
        assert_eq!(layered.max_depth, 3);
        assert_eq!(layered.max_manifest_bytes, defaults.max_manifest_bytes);
        assert!(Limits::from_toml_str("unknown = 1").is_err());
    }
}
