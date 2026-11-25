//! Key nicknames of a provider and the one the user made active. No key value passes here.

use std::fmt;

use crate::config::{ConfigError, invalid};

/// Most nicknames one provider may list.
pub const MAX_KEY_ALIASES: usize = 8;
/// Longest nickname, in characters.
pub const MAX_KEY_ALIAS_CHARS: usize = 32;

// A run this long, with digits and no separator, reads as key material rather than a name.
const OPAQUE_RUN_CHARS: usize = 16;
const SECRET_PREFIXES: [&str; 7] = ["sk-", "sk_", "rk_", "gsk_", "ghp_", "hf_", "xai-"];

/// Validated nicknames, never empty, with the active one among them.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyAliases {
    aliases: Vec<String>,
    active: usize,
}

// Redacted on purpose: key references must not reach a log through `{:?}`.
impl fmt::Debug for KeyAliases {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyAliases(<redacted>)")
    }
}

impl KeyAliases {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.aliases.iter().map(String::as_str)
    }

    pub fn active(&self) -> &str {
        &self.aliases[self.active]
    }

    /// `active` may be omitted only when there is a single nickname to choose.
    pub(crate) fn select(
        field: &str,
        aliases: Vec<String>,
        active: Option<&toml::Value>,
    ) -> Result<Self, ConfigError> {
        let active = match active {
            None if aliases.len() == 1 => 0,
            None => {
                return Err(invalid(
                    field,
                    "required when there is more than one key alias",
                ));
            }
            Some(value) => {
                let wanted = alias(field, value)?;
                aliases
                    .iter()
                    .position(|a| a == wanted)
                    .ok_or_else(|| invalid(field, "is not one of the key aliases"))?
            }
        };
        Ok(Self { aliases, active })
    }
}

/// Read the list as an untyped value: a typed read would quote a wrong value in its error.
pub(crate) fn alias_list(field: &str, value: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let items = value
        .as_array()
        .ok_or_else(|| invalid(field, "must be a list of key aliases"))?;
    if items.is_empty() || items.len() > MAX_KEY_ALIASES {
        return Err(invalid(
            field,
            format!("needs 1 to {MAX_KEY_ALIASES} key aliases"),
        ));
    }
    let mut aliases: Vec<String> = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let at = format!("{field}[{index}]");
        let name = alias(&at, item)?;
        if aliases.iter().any(|a| a == name) {
            return Err(invalid(at, "repeats an earlier key alias"));
        }
        aliases.push(name.to_owned());
    }
    Ok(aliases)
}

/// No error here repeats the value: it may be a key pasted in the wrong place.
fn alias<'a>(field: &str, value: &'a toml::Value) -> Result<&'a str, ConfigError> {
    let name = value
        .as_str()
        .ok_or_else(|| invalid(field, "must be a text key alias"))?;
    if looks_like_secret(name) {
        return Err(invalid(
            field,
            "looks like a secret; use a short nickname, never the key value",
        ));
    }
    // Lowercase only: vault target names may ignore case.
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_');
    if !name.starts_with(|c: char| c.is_ascii_lowercase())
        || name.len() > MAX_KEY_ALIAS_CHARS
        || !name.chars().all(allowed)
    {
        return Err(invalid(
            field,
            format!(
                "must be 1 to {MAX_KEY_ALIAS_CHARS} lowercase letters, digits, - or _, starting with a letter"
            ),
        ));
    }
    Ok(name)
}

fn looks_like_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SECRET_PREFIXES.iter().any(|p| lower.starts_with(p))
        || lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|run| run.len() >= OPAQUE_RUN_CHARS && run.bytes().any(|b| b.is_ascii_digit()))
}
