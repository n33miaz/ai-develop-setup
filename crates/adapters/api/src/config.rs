//! Adapter configuration: the documented defaults plus per-provider settings, validated.

use std::collections::BTreeMap;
use std::fmt;

use ads_contracts::CommonIdV1;
use serde::Deserialize;

use crate::chat::KeyHandle;
use crate::keys::{KeyAliases, alias_list, looks_like_secret};

/// Documented defaults, embedded so a missing key always has a value.
pub const DEFAULT_POLICY: &str = include_str!("../../../../defaults/api-adapter.toml");

const MAX_MODEL_CHARS: usize = 128;
const MAX_PRICE_VERSION_CHARS: usize = 96;
const MICRO_PER_USD: u64 = 1_000_000;
const MAX_USD: u64 = 1_000_000_000;
const HEADER_TOKEN_SYMBOLS: &str = "!#$%&'*+-.^_`|~";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub field: String,
    pub reason: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.reason)
    }
}

impl std::error::Error for ConfigError {}

pub(crate) fn invalid(field: impl Into<String>, reason: impl Into<String>) -> ConfigError {
    ConfigError {
        field: field.into(),
        reason: reason.into(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    enabled: bool,
    monthly_cap_usd: Option<f64>,
    chat_path: String,
    key_header: String,
    key_prefix: String,
    max_output_tokens: u32,
    key_aliases: toml::Value,
}

#[derive(Deserialize)]
struct ConfigLayer {
    providers: Option<BTreeMap<String, RawProvider>>,
    #[serde(flatten)]
    settings: toml::Table,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProvider {
    base_url: String,
    model: String,
    input_usd_per_mtok: toml::Spanned<toml::Value>,
    output_usd_per_mtok: toml::Spanned<toml::Value>,
    price_version: String,
    chat_path: Option<String>,
    key_header: Option<String>,
    key_prefix: Option<String>,
    max_output_tokens: Option<u32>,
    key_aliases: Option<toml::Value>,
    active_key: Option<toml::Value>,
}

/// Settings of one provider that speaks the Chat Completions format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    alias: CommonIdV1,
    pub url: String,
    pub model: String,
    pub key_header: String,
    pub key_prefix: String,
    pub max_output_tokens: u32,
    pub input_micro_usd_per_mtok: u64,
    pub output_micro_usd_per_mtok: u64,
    pub price_version: String,
    pub keys: KeyAliases,
}

impl ProviderConfig {
    pub fn alias(&self) -> &CommonIdV1 {
        &self.alias
    }

    /// Vault reference of the active key, `<provider>/<key alias>`.
    pub fn key_handle(&self) -> KeyHandle {
        KeyHandle::new(format!("{}/{}", self.alias.as_str(), self.keys.active()))
    }

    /// Cost in micro-dollars, rounded up so an estimate never undercounts.
    pub fn cost_micro_usd(&self, tokens_in: u64, tokens_out: u64) -> u64 {
        let total = u128::from(tokens_in) * u128::from(self.input_micro_usd_per_mtok)
            + u128::from(tokens_out) * u128::from(self.output_micro_usd_per_mtok);
        u64::try_from(total.div_ceil(1_000_000)).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiConfig {
    pub enabled: bool,
    pub monthly_cap_micro_usd: Option<u64>,
    providers: BTreeMap<String, ProviderConfig>,
}

impl ApiConfig {
    /// The documented defaults alone: off, no cap, no provider.
    pub fn defaults() -> Self {
        Self::from_toml("").expect("embedded defaults are valid")
    }

    /// Layer the user's top-level keys over the defaults, then validate everything.
    pub fn from_toml(user: &str) -> Result<Self, ConfigError> {
        let defaults: ConfigLayer = toml::from_str(DEFAULT_POLICY)
            .map_err(|_| invalid("defaults", "embedded defaults do not parse"))?;
        let overrides: ConfigLayer =
            toml::from_str(user).map_err(|_| invalid("config", "invalid configuration"))?;
        let mut table = defaults.settings;
        table.extend(overrides.settings);
        let raw: Raw = table
            .try_into()
            .map_err(|_| invalid("config", "unknown field, missing field or invalid value"))?;
        let (providers, source) = match overrides.providers {
            Some(providers) => (providers, user),
            None => (defaults.providers.unwrap_or_default(), DEFAULT_POLICY),
        };
        Self::validate(raw, providers, source)
    }

    fn validate(
        raw: Raw,
        entries: BTreeMap<String, RawProvider>,
        source: &str,
    ) -> Result<Self, ConfigError> {
        let cap = match raw.monthly_cap_usd {
            None => None,
            Some(usd) => {
                if !usd.is_finite() || usd <= 0.0 || usd > MAX_USD as f64 {
                    return Err(invalid("monthly_cap_usd", "must be greater than zero"));
                }
                let micro = (usd * MICRO_PER_USD as f64).floor() as u64;
                if micro == 0 {
                    return Err(invalid("monthly_cap_usd", "below one micro-dollar"));
                }
                Some(micro)
            }
        };
        check_path("chat_path", &raw.chat_path)?;
        check_header_name("key_header", &raw.key_header)?;
        check_prefix("key_prefix", &raw.key_prefix)?;
        check_max_tokens("max_output_tokens", raw.max_output_tokens)?;
        let default_aliases = alias_list("key_aliases", &raw.key_aliases)?;

        let mut providers = BTreeMap::new();
        for (name, p) in entries {
            if looks_like_secret(&name) {
                return Err(invalid(
                    "providers",
                    "use a provider alias, never a key value",
                ));
            }
            let alias = CommonIdV1::new(name.clone())
                .map_err(|_| invalid("providers", "alias is not a valid id"))?;
            let at = |key: &str| format!("providers.{name}.{key}");
            let chat_path = p.chat_path.unwrap_or_else(|| raw.chat_path.clone());
            let key_header = p.key_header.unwrap_or_else(|| raw.key_header.clone());
            let key_prefix = p.key_prefix.unwrap_or_else(|| raw.key_prefix.clone());
            let max_output_tokens = p.max_output_tokens.unwrap_or(raw.max_output_tokens);
            check_path(&at("chat_path"), &chat_path)?;
            check_header_name(&at("key_header"), &key_header)?;
            check_prefix(&at("key_prefix"), &key_prefix)?;
            check_max_tokens(&at("max_output_tokens"), max_output_tokens)?;
            if p.model.is_empty()
                || p.model.chars().count() > MAX_MODEL_CHARS
                || p.model.chars().any(char::is_control)
            {
                return Err(invalid(
                    at("model"),
                    "empty, too long or has control characters",
                ));
            }
            if p.price_version.is_empty()
                || p.price_version.chars().count() > MAX_PRICE_VERSION_CHARS
            {
                return Err(invalid(at("price_version"), "empty or too long"));
            }
            let key_aliases = match &p.key_aliases {
                Some(list) => alias_list(&at("key_aliases"), list)?,
                None => default_aliases.clone(),
            };
            let keys = KeyAliases::select(&at("active_key"), key_aliases, p.active_key.as_ref())?;
            let base = check_base_url(&at("base_url"), &p.base_url)?;
            let input = price(&at("input_usd_per_mtok"), p.input_usd_per_mtok, source)?;
            let output = price(&at("output_usd_per_mtok"), p.output_usd_per_mtok, source)?;
            providers.insert(
                name,
                ProviderConfig {
                    alias,
                    url: format!("{base}{chat_path}"),
                    model: p.model,
                    key_header,
                    key_prefix,
                    max_output_tokens,
                    input_micro_usd_per_mtok: input,
                    output_micro_usd_per_mtok: output,
                    price_version: p.price_version,
                    keys,
                },
            );
        }
        Ok(Self {
            enabled: raw.enabled,
            monthly_cap_micro_usd: cap,
            providers,
        })
    }

    pub fn provider(&self, alias: &str) -> Option<&ProviderConfig> {
        self.providers.get(alias)
    }

    pub fn providers(&self) -> impl Iterator<Item = &ProviderConfig> {
        self.providers.values()
    }
}

fn price(field: &str, rate: toml::Spanned<toml::Value>, source: &str) -> Result<u64, ConfigError> {
    let micro = match rate.get_ref() {
        toml::Value::Integer(usd) => u64::try_from(*usd)
            .ok()
            .and_then(|n| n.checked_mul(MICRO_PER_USD)),
        // The binary float can lose decimal places before we even scale it.
        toml::Value::Float(n) if n.is_finite() => decimal_micro_usd(&source[rate.span()]),
        _ => None,
    };
    micro.filter(|n| *n <= MAX_USD * MICRO_PER_USD)
        .ok_or_else(|| invalid(field, "must be a non-negative price up to one billion USD in whole micro-dollars per million tokens"))
}

fn decimal_micro_usd(literal: &str) -> Option<u64> {
    let literal = literal.replace('_', "");
    let (mantissa, exponent) = literal.split_once(['e', 'E']).unwrap_or((&literal, "0"));
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let unsigned = digits.trim_start_matches(['+', '-']);
    let significant = unsigned.trim_end_matches('0');
    if significant.is_empty() {
        return Some(0);
    }
    if mantissa.starts_with('-') {
        return None;
    }
    let trailing_zeros = unsigned.len() - significant.len();
    let scale = 6 + i64::from(exponent.parse::<i32>().ok()?) + trailing_zeros as i64
        - fraction.len() as i64;
    let factor = 10_u64.checked_pow(scale.try_into().ok()?)?;
    significant.parse::<u64>().ok()?.checked_mul(factor)
}

fn check_max_tokens(field: &str, value: u32) -> Result<(), ConfigError> {
    if value == 0 {
        return Err(invalid(field, "must be at least 1"));
    }
    Ok(())
}

fn check_path(field: &str, path: &str) -> Result<(), ConfigError> {
    let bad = |c: char| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#');
    if !path.starts_with('/') || path.chars().any(bad) {
        return Err(invalid(
            field,
            "must start with / and have no query, fragment or spaces",
        ));
    }
    Ok(())
}

fn check_header_name(field: &str, name: &str) -> Result<(), ConfigError> {
    let token = |c: char| c.is_ascii_alphanumeric() || HEADER_TOKEN_SYMBOLS.contains(c);
    if name.is_empty() || !name.chars().all(token) {
        return Err(invalid(field, "not a valid header name"));
    }
    Ok(())
}

fn check_prefix(field: &str, prefix: &str) -> Result<(), ConfigError> {
    if prefix.chars().any(char::is_control) {
        return Err(invalid(field, "has control characters"));
    }
    Ok(())
}

/// HTTPS only, no embedded credentials, query or fragment; returns it without a trailing slash.
fn check_base_url(field: &str, url: &str) -> Result<String, ConfigError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| invalid(field, "must start with https://"))?;
    let authority = rest.split('/').next().unwrap_or_default();
    let bad = |c: char| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#' | '\\');
    if authority.is_empty() || authority.contains('@') || url.chars().any(bad) {
        return Err(invalid(
            field,
            "needs a host and no credentials, query, fragment or spaces",
        ));
    }
    Ok(url.trim_end_matches('/').to_owned())
}
