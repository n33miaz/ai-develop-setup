//! Adapter configuration: the documented defaults plus per-provider settings, validated.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroUsize;

use ads_contracts::CommonIdV1;
use serde::de::DeserializeOwned;
use toml::Spanned;
use toml::de::{DeTable, DeValue, ValueDeserializer};

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

struct Raw {
    enabled: bool,
    monthly_cap_usd: Option<f64>,
    chat_path: String,
    key_header: String,
    key_prefix: String,
    max_output_tokens: u32,
    max_request_bytes: NonZeroUsize,
    key_aliases: toml::Value,
}

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
    max_request_bytes: Option<NonZeroUsize>,
    key_aliases: Option<toml::Value>,
    active_key: Option<toml::Value>,
}

struct Settings<'a> {
    table: DeTable<'a>,
    scope: Option<&'a str>,
}

impl Settings<'_> {
    fn field(&self, key: &'static str) -> String {
        match self.scope {
            Some(scope) => format!("{scope}.{key}"),
            None => key.to_owned(),
        }
    }

    fn optional<T: DeserializeOwned>(
        &mut self,
        key: &'static str,
    ) -> Result<Option<T>, ConfigError> {
        take(&mut self.table, key)
            .map(|value| {
                T::deserialize(ValueDeserializer::from(value))
                    .map_err(|_| invalid(self.field(key), "invalid value"))
            })
            .transpose()
    }

    fn required<T: DeserializeOwned>(&mut self, key: &'static str) -> Result<T, ConfigError> {
        self.optional(key)?
            .ok_or_else(|| invalid(self.field(key), "missing field"))
    }

    fn finish(&self) -> Result<(), ConfigError> {
        if !self.table.is_empty() {
            return Err(invalid(self.scope.unwrap_or("config"), "unknown field"));
        }
        Ok(())
    }
}

impl Raw {
    fn read(table: DeTable<'_>) -> Result<Self, ConfigError> {
        let mut settings = Settings { table, scope: None };
        let raw = Self {
            enabled: settings.required("enabled")?,
            monthly_cap_usd: settings.optional("monthly_cap_usd")?,
            chat_path: settings.required("chat_path")?,
            key_header: settings.required("key_header")?,
            key_prefix: settings.required("key_prefix")?,
            max_output_tokens: settings.required("max_output_tokens")?,
            max_request_bytes: settings.required("max_request_bytes")?,
            key_aliases: settings.required("key_aliases")?,
        };
        settings.finish()?;
        Ok(raw)
    }
}

impl RawProvider {
    fn read(table: DeTable<'_>, scope: &str) -> Result<Self, ConfigError> {
        let mut settings = Settings {
            table,
            scope: Some(scope),
        };
        let raw = Self {
            base_url: settings.required("base_url")?,
            model: settings.required("model")?,
            input_usd_per_mtok: settings.required("input_usd_per_mtok")?,
            output_usd_per_mtok: settings.required("output_usd_per_mtok")?,
            price_version: settings.required("price_version")?,
            chat_path: settings.optional("chat_path")?,
            key_header: settings.optional("key_header")?,
            key_prefix: settings.optional("key_prefix")?,
            max_output_tokens: settings.optional("max_output_tokens")?,
            max_request_bytes: settings.optional("max_request_bytes")?,
            key_aliases: settings.optional("key_aliases")?,
            active_key: settings.optional("active_key")?,
        };
        settings.finish()?;
        Ok(raw)
    }
}

fn take<'a>(table: &mut DeTable<'a>, key: &'static str) -> Option<Spanned<DeValue<'a>>> {
    table.remove(&Spanned::new(0..0, key.into()))
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
    pub max_request_bytes: usize,
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
        let mut defaults = DeTable::parse(DEFAULT_POLICY)
            .map(Spanned::into_inner)
            .map_err(|_| invalid("defaults", "embedded defaults do not parse"))?;
        let mut overrides = DeTable::parse(user)
            .map(Spanned::into_inner)
            .map_err(|error| {
                let reason = match error.span() {
                    Some(span) => {
                        let line = 1 + user
                            .bytes()
                            .take(span.start)
                            .filter(|&b| b == b'\n')
                            .count();
                        format!("invalid TOML at line {line}")
                    }
                    None => "invalid TOML".to_owned(),
                };
                invalid("config", reason)
            })?;
        let default_providers = take(&mut defaults, "providers");
        let (providers, source) = match take(&mut overrides, "providers") {
            Some(providers) => (Some(providers), user),
            None => (default_providers, DEFAULT_POLICY),
        };
        defaults.extend(overrides);
        let raw = Raw::read(defaults)?;
        let entries = match providers.map(Spanned::into_inner) {
            Some(DeValue::Table(entries)) => entries,
            Some(_) => return Err(invalid("providers", "must be a table")),
            None => DeTable::new(),
        };
        Self::validate(raw, entries, source)
    }

    fn validate(raw: Raw, entries: DeTable<'_>, source: &str) -> Result<Self, ConfigError> {
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
        let mut entries: Vec<_> = entries.into_iter().collect();
        entries.sort_by_key(|(name, _)| name.span().start);
        for (index, (name, entry)) in entries.into_iter().enumerate() {
            let position = index + 1;
            let scope = format!("providers[{position}]");
            let name = name.into_inner().into_owned();
            if looks_like_secret(&name) {
                return Err(invalid(scope, "use a provider alias, never a key value"));
            }
            let alias = CommonIdV1::new(name.clone())
                .map_err(|_| invalid(&scope, "alias is not a valid id"))?;
            let DeValue::Table(table) = entry.into_inner() else {
                return Err(invalid(scope, "must be a table"));
            };
            let p = RawProvider::read(table, &scope)?;
            let at = |key: &str| format!("{scope}.{key}");
            let chat_path = p.chat_path.unwrap_or_else(|| raw.chat_path.clone());
            let key_header = p.key_header.unwrap_or_else(|| raw.key_header.clone());
            let key_prefix = p.key_prefix.unwrap_or_else(|| raw.key_prefix.clone());
            let max_output_tokens = p.max_output_tokens.unwrap_or(raw.max_output_tokens);
            let max_request_bytes = p.max_request_bytes.unwrap_or(raw.max_request_bytes).get();
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
                    max_request_bytes,
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
