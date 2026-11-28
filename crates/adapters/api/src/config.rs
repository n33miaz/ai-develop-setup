//! Adapter configuration: the documented defaults plus per-provider settings, validated.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroUsize;

use ads_contracts::CommonIdV1;
use ads_core::spend::{CapAboveMaximum, MAX_MONTHLY_CAP_MICRO_USD, SpendLimit};
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
const MONTHLY_CAP_FIELD: &str = "monthly_cap_usd";
const HEADER_TOKEN_SYMBOLS: &str = "!#$%&'*+-.^_`|~";
const CHAT_PATH_EXPECTED: &str = "must be a path starting with / without query, fragment or spaces";
const KEY_HEADER_EXPECTED: &str = "must be a valid HTTP header name";
const KEY_PREFIX_EXPECTED: &str = "must be text without control characters";
const MAX_OUTPUT_TOKENS_EXPECTED: &str = "must be an integer from 1 to 4294967295";
const MAX_REQUEST_BYTES_EXPECTED: &str = "must be a positive integer in bytes";
const MONTHLY_CAP_EXPECTED: &str = "must be a finite number";
const PRICE_EXPECTED: &str =
    "must be a non-negative price up to one billion USD in whole micro-dollars per million tokens";

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
    monthly_cap_usd: Option<Spanned<toml::Value>>,
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
        expected: &str,
    ) -> Result<Option<T>, ConfigError> {
        take(&mut self.table, key)
            .map(|value| {
                T::deserialize(ValueDeserializer::from(value))
                    .map_err(|_| invalid(self.field(key), expected))
            })
            .transpose()
    }

    fn required<T: DeserializeOwned>(
        &mut self,
        key: &'static str,
        expected: &str,
    ) -> Result<T, ConfigError> {
        self.optional(key, expected)?
            .ok_or_else(|| invalid(self.field(key), "missing field"))
    }

    fn finish(&self, source: &str) -> Result<(), ConfigError> {
        if let Some(key) = self.table.keys().min_by_key(|key| key.span().start) {
            let line = line_at(source, key.span().start);
            return Err(invalid(
                self.scope.unwrap_or("config"),
                format!("unknown field at line {line}"),
            ));
        }
        Ok(())
    }
}

impl Raw {
    fn read(table: DeTable<'_>, source: &str) -> Result<Self, ConfigError> {
        let mut settings = Settings { table, scope: None };
        let raw = Self {
            enabled: settings.required("enabled", "must be a boolean")?,
            monthly_cap_usd: settings.optional(MONTHLY_CAP_FIELD, MONTHLY_CAP_EXPECTED)?,
            chat_path: settings.required("chat_path", CHAT_PATH_EXPECTED)?,
            key_header: settings.required("key_header", KEY_HEADER_EXPECTED)?,
            key_prefix: settings.required("key_prefix", KEY_PREFIX_EXPECTED)?,
            max_output_tokens: settings
                .required("max_output_tokens", MAX_OUTPUT_TOKENS_EXPECTED)?,
            max_request_bytes: settings
                .required("max_request_bytes", MAX_REQUEST_BYTES_EXPECTED)?,
            key_aliases: settings.required("key_aliases", "must be a list of key aliases")?,
        };
        settings.finish(source)?;
        Ok(raw)
    }
}

impl RawProvider {
    fn read(table: DeTable<'_>, scope: &str, source: &str) -> Result<Self, ConfigError> {
        let mut settings = Settings {
            table,
            scope: Some(scope),
        };
        let raw = Self {
            base_url: settings.required("base_url", "must be an HTTPS URL")?,
            model: settings.required("model", "must be non-empty text")?,
            input_usd_per_mtok: settings.required("input_usd_per_mtok", PRICE_EXPECTED)?,
            output_usd_per_mtok: settings.required("output_usd_per_mtok", PRICE_EXPECTED)?,
            price_version: settings.required("price_version", "must be non-empty text")?,
            chat_path: settings.optional("chat_path", CHAT_PATH_EXPECTED)?,
            key_header: settings.optional("key_header", KEY_HEADER_EXPECTED)?,
            key_prefix: settings.optional("key_prefix", KEY_PREFIX_EXPECTED)?,
            max_output_tokens: settings
                .optional("max_output_tokens", MAX_OUTPUT_TOKENS_EXPECTED)?,
            max_request_bytes: settings
                .optional("max_request_bytes", MAX_REQUEST_BYTES_EXPECTED)?,
            key_aliases: settings.optional("key_aliases", "must be a list of key aliases")?,
            active_key: settings.optional("active_key", "must be a text key alias")?,
        };
        settings.finish(source)?;
        Ok(raw)
    }
}

fn take<'a>(table: &mut DeTable<'a>, key: &'static str) -> Option<Spanned<DeValue<'a>>> {
    table.remove(&Spanned::new(0..0, key.into()))
}

fn line_at(source: &str, offset: usize) -> usize {
    1 + source.bytes().take(offset).filter(|&b| b == b'\n').count()
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
    pub(crate) spend_limit: SpendLimit,
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
                    Some(span) => format!("invalid TOML at line {}", line_at(user, span.start)),
                    None => "invalid TOML".to_owned(),
                };
                invalid("config", reason)
            })?;
        let default_providers = take(&mut defaults, "providers");
        let (providers, source) = match take(&mut overrides, "providers") {
            Some(providers) => (Some(providers), user),
            None => (default_providers, DEFAULT_POLICY),
        };
        let cap_source = if overrides.contains_key(&Spanned::new(0..0, MONTHLY_CAP_FIELD.into())) {
            user
        } else {
            DEFAULT_POLICY
        };
        defaults.extend(overrides);
        let raw = Raw::read(defaults, user)?;
        let cap = raw
            .monthly_cap_usd
            .as_ref()
            .map(|value| monthly_cap(value, cap_source))
            .transpose()?;
        let spend_limit = SpendLimit::new(raw.enabled, cap).map_err(cap_above_maximum)?;
        let entries = match providers.map(Spanned::into_inner) {
            Some(DeValue::Table(entries)) => entries,
            Some(_) => return Err(invalid("providers", "must be a table")),
            None => DeTable::new(),
        };
        Self::validate(raw, entries, source, spend_limit)
    }

    fn validate(
        raw: Raw,
        entries: DeTable<'_>,
        source: &str,
        spend_limit: SpendLimit,
    ) -> Result<Self, ConfigError> {
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
            let p = RawProvider::read(table, &scope, source)?;
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
                    format!("must be 1 to {MAX_MODEL_CHARS} characters without control characters"),
                ));
            }
            if p.price_version.is_empty()
                || p.price_version.chars().count() > MAX_PRICE_VERSION_CHARS
            {
                return Err(invalid(
                    at("price_version"),
                    format!("must be 1 to {MAX_PRICE_VERSION_CHARS} characters"),
                ));
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
            spend_limit,
            providers,
        })
    }

    pub fn enabled(&self) -> bool {
        self.spend_limit.enabled()
    }

    pub fn monthly_cap_micro_usd(&self) -> Option<u64> {
        self.spend_limit.monthly_cap_micro_usd()
    }

    pub fn provider(&self, alias: &str) -> Option<&ProviderConfig> {
        self.providers.get(alias)
    }

    pub fn providers(&self) -> impl Iterator<Item = &ProviderConfig> {
        self.providers.values()
    }
}

fn price(field: &str, rate: toml::Spanned<toml::Value>, source: &str) -> Result<u64, ConfigError> {
    micro_usd(&rate, source)
        .filter(|&(whole, fractional)| !fractional && whole <= MAX_USD * MICRO_PER_USD)
        .map(|(whole, _)| whole)
        .ok_or_else(|| invalid(field, PRICE_EXPECTED))
}

fn cap_above_maximum(error: CapAboveMaximum) -> ConfigError {
    invalid(MONTHLY_CAP_FIELD, error.to_string())
}

fn monthly_cap(value: &Spanned<toml::Value>, source: &str) -> Result<u64, ConfigError> {
    let negative = match value.get_ref() {
        toml::Value::Integer(n) => *n < 0,
        toml::Value::Float(n) => n.is_finite() && n.is_sign_negative(),
        _ => false,
    };
    let error = |reason| invalid(MONTHLY_CAP_FIELD, reason);
    if negative {
        return Err(error("must be greater than zero"));
    }
    match micro_usd(value, source) {
        None => Err(error(MONTHLY_CAP_EXPECTED)),
        Some((0, false)) => Err(error("must be greater than zero")),
        Some((0, true)) => Err(error("below one micro-dollar")),
        Some((whole, true)) if whole >= MAX_MONTHLY_CAP_MICRO_USD => {
            Err(cap_above_maximum(CapAboveMaximum))
        }
        Some((_, true)) => Err(error("must be in whole micro-dollars")),
        Some((whole, false)) => Ok(whole),
    }
}

fn micro_usd(value: &Spanned<toml::Value>, source: &str) -> Option<(u64, bool)> {
    match value.get_ref() {
        toml::Value::Integer(usd) => {
            let usd = u64::try_from(*usd).ok()?;
            Some((usd.saturating_mul(MICRO_PER_USD), false))
        }
        // The binary float can lose decimal places before we even scale it.
        toml::Value::Float(_) => decimal_micro_usd(&source[value.span()]),
        _ => None,
    }
}

// Return the whole micro-dollars (saturating on overflow) and whether a fraction remains.
fn decimal_micro_usd(literal: &str) -> Option<(u64, bool)> {
    let literal = literal.replace('_', "");
    let (mantissa, exponent) = literal.split_once(['e', 'E']).unwrap_or((&literal, "0"));
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let unsigned = digits
        .trim_start_matches(['+', '-'])
        .trim_start_matches('0');
    let significant = unsigned.trim_end_matches('0');
    if significant.is_empty() {
        return Some((0, false));
    }
    if mantissa.starts_with('-') || !significant.as_bytes()[0].is_ascii_digit() {
        return None;
    }
    let trailing_zeros = unsigned.len() - significant.len();
    let exponent = exponent
        .parse::<i32>()
        .unwrap_or(if exponent.starts_with('-') {
            i32::MIN
        } else {
            i32::MAX
        });
    let scale = 6 + i64::from(exponent) + trailing_zeros as i64 - fraction.len() as i64;
    let whole_digits = significant.len() as i64 + scale;
    if whole_digits <= 0 {
        return Some((0, true));
    }
    if whole_digits > 20 {
        return Some((u64::MAX, false));
    }
    let prefix_len = significant.len().min(whole_digits as usize);
    let whole = significant[..prefix_len].parse::<u64>().unwrap_or(u64::MAX);
    let zeros = whole_digits as u32 - prefix_len as u32;
    Some((
        whole.saturating_mul(10_u64.saturating_pow(zeros)),
        prefix_len < significant.len(),
    ))
}

fn check_max_tokens(field: &str, value: u32) -> Result<(), ConfigError> {
    if value == 0 {
        return Err(invalid(field, MAX_OUTPUT_TOKENS_EXPECTED));
    }
    Ok(())
}

fn check_path(field: &str, path: &str) -> Result<(), ConfigError> {
    let bad = |c: char| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#');
    if !path.starts_with('/') || path.chars().any(bad) {
        return Err(invalid(field, CHAT_PATH_EXPECTED));
    }
    Ok(())
}

fn check_header_name(field: &str, name: &str) -> Result<(), ConfigError> {
    let token = |c: char| c.is_ascii_alphanumeric() || HEADER_TOKEN_SYMBOLS.contains(c);
    if name.is_empty() || !name.chars().all(token) {
        return Err(invalid(field, KEY_HEADER_EXPECTED));
    }
    Ok(())
}

fn check_prefix(field: &str, prefix: &str) -> Result<(), ConfigError> {
    if prefix.chars().any(char::is_control) {
        return Err(invalid(field, KEY_PREFIX_EXPECTED));
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
