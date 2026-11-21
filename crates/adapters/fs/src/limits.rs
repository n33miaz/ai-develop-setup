//! Parameters of the executor. The shipped values live only in `defaults/fs-executor.toml`.

use serde::Deserialize;

const DEFAULTS_TOML: &str = include_str!("../../../../defaults/fs-executor.toml");

/// When an entry may be removed for good (not moved to quarantine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitiveDelete {
    /// Only where the system can hold a folder in place while its entries are removed
    /// (Windows). Elsewhere (Linux) every definitive delete is refused.
    Contained,
    /// Everywhere. Where folders cannot be held in place, another process of the same user
    /// can take a folder that is being emptied out of the fence, and removals follow it.
    Isolated,
    /// Never.
    Refuse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// How many folders deep a tree may go before the executor refuses it.
    pub max_depth: u32,
    /// The maximum entries examined across one plan inspection.
    pub max_plan_entries: usize,
    /// The largest manifest the executor writes or reads back, in bytes.
    pub max_manifest_bytes: u64,
    pub definitive_delete: DefinitiveDelete,
    /// The names, below the work root, of the folder where a tree is held while it is
    /// removed for good.
    pub private_dir: Vec<String>,
}

#[derive(Debug)]
pub enum LimitsError {
    Toml(toml::de::Error),
    MissingKey(&'static str),
    InvalidName { name: String, error: std::io::Error },
}

impl std::fmt::Display for LimitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(error) => error.fmt(f),
            Self::MissingKey(key) => write!(f, "missing key `{key}`"),
            Self::InvalidName { name, .. } => {
                write!(f, "`private_dir` has a name that is not plain: `{name}`")
            }
        }
    }
}

impl std::error::Error for LimitsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toml(error) => Some(error),
            Self::InvalidName { error, .. } => Some(error),
            Self::MissingKey(_) => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    max_depth: Option<u32>,
    max_plan_entries: Option<usize>,
    max_manifest_bytes: Option<u64>,
    definitive_delete: Option<DefinitiveDelete>,
    private_dir: Option<String>,
}

impl Limits {
    pub fn defaults() -> Self {
        Self::from_toml_str("").expect("the shipped defaults are complete")
    }

    /// The shipped defaults with the keys of `overrides` applied.
    pub fn from_toml_str(overrides: &str) -> Result<Self, LimitsError> {
        let parse = |text: &str| toml::from_str::<Layer>(text).map_err(LimitsError::Toml);
        let base = parse(DEFAULTS_TOML)?;
        let over = parse(overrides)?;
        let private_dir = over
            .private_dir
            .or(base.private_dir)
            .ok_or(LimitsError::MissingKey("private_dir"))?;
        let private_dir: Vec<String> = private_dir.split('/').map(str::to_owned).collect();
        for name in &private_dir {
            crate::sys::plain(name).map_err(|error| LimitsError::InvalidName {
                name: name.clone(),
                error,
            })?;
        }
        Ok(Self {
            private_dir,
            definitive_delete: over
                .definitive_delete
                .or(base.definitive_delete)
                .ok_or(LimitsError::MissingKey("definitive_delete"))?,
            max_depth: over
                .max_depth
                .or(base.max_depth)
                .ok_or(LimitsError::MissingKey("max_depth"))?,
            max_plan_entries: over
                .max_plan_entries
                .or(base.max_plan_entries)
                .ok_or(LimitsError::MissingKey("max_plan_entries"))?,
            max_manifest_bytes: over
                .max_manifest_bytes
                .or(base.max_manifest_bytes)
                .ok_or(LimitsError::MissingKey("max_manifest_bytes"))?,
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
        // The shipped default is the strictest one that still deletes somewhere.
        assert_eq!(defaults.definitive_delete, DefinitiveDelete::Contained);
        assert_eq!(defaults.private_dir, [".ads", "tmp"]);
        let layered = Limits::from_toml_str("definitive_delete = \"refuse\"").unwrap();
        assert_eq!(layered.definitive_delete, DefinitiveDelete::Refuse);
        for bad in ["", "a//b", "../x", "a/..", "/abs"] {
            let layer = format!("private_dir = \"{bad}\"");
            assert!(Limits::from_toml_str(&layer).is_err(), "{bad}");
        }
        assert!(Limits::from_toml_str("definitive_delete = \"in_place\"").is_err());
    }
}
