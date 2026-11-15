// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.usage_event.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static RECORD_USAGE_EVENT_V1_RULES: &[(&str, Rule)] = &[(
    "#/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[(
                "status",
                Rule::Enum(&[Literal::Str("error"), Literal::Str("429")]),
            )]),
            Rule::Required(&["status"]),
        ]),
        then: Some(&Rule::Properties(&[(
            "error_code",
            Rule::Type(&[JsonType::String]),
        )])),
        otherwise: None,
    },
)];

/// Evento de uso. Uma linha do livro-caixa: uma chamada ou turno de IA medido, com tokens, duração, custo de lista, custo faturável (só chave de API), janela de cota, fonte e chave de deduplicação. Só-anexo; alias de conta nunca é credencial nem e-mail.
///
/// `urn:ads:record:usage_event:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordUsageEventV1Fields")]
pub struct RecordUsageEventV1(RecordUsageEventV1Fields);

/// The fields of [`RecordUsageEventV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUsageEventV1Fields {
    pub version: RecordUsageEventV1Version,
    pub event_id: CommonIdV1,
    pub ts: CommonTimestampV1,
    #[serde(deserialize_with = "support::nullable")]
    pub task_id: Option<CommonIdV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub parent_task_id: Option<CommonIdV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub session_id: Option<CommonIdV1>,
    pub workspace: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub project_id: Option<CommonIdV1>,
    pub os_user: CommonIdV1,
    pub role: RecordUsageEventV1Role,
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub account_alias: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub model_requested: Option<RecordUsageEventV1ModelRequested>,
    pub model_effective: RecordUsageEventV1ModelEffective,
    #[serde(deserialize_with = "support::nullable")]
    pub effort_requested: Option<RecordUsageEventV1EffortRequested>,
    #[serde(deserialize_with = "support::nullable")]
    pub effort_effective: Option<RecordUsageEventV1EffortEffective>,
    #[serde(deserialize_with = "support::nullable")]
    pub routing_mode: Option<RecordUsageEventV1RoutingMode>,
    #[serde(deserialize_with = "support::nullable")]
    pub routing_rule_id: Option<CommonIdV1>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tokens_cache_read: u64,
    pub tokens_cache_write: u64,
    pub duration_ms: u64,
    pub status: RecordUsageEventV1Status,
    #[serde(deserialize_with = "support::nullable")]
    pub error_code: Option<RecordUsageEventV1ErrorCode>,
    pub price_version: RecordUsageEventV1PriceVersion,
    #[serde(deserialize_with = "support::nullable")]
    pub cost_list_usd: Option<RecordUsageEventV1CostListUsd>,
    #[serde(deserialize_with = "support::nullable")]
    pub cost_billable_usd: Option<RecordUsageEventV1CostBillableUsd>,
    #[serde(deserialize_with = "support::nullable")]
    pub quota_window_before: Option<RecordUsageEventV1QuotaWindowBefore>,
    #[serde(deserialize_with = "support::nullable")]
    pub quota_window_after: Option<RecordUsageEventV1QuotaWindowAfter>,
    pub source: RecordUsageEventV1Source,
    pub source_ref: RecordUsageEventV1SourceRef,
    pub dedupe_key: RecordUsageEventV1DedupeKey,
}

impl RecordUsageEventV1 {
    pub fn new(fields: RecordUsageEventV1Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordUsageEventV1", &fields, RECORD_USAGE_EVENT_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordUsageEventV1Fields {
        self.0
    }
}

impl Deref for RecordUsageEventV1 {
    type Target = RecordUsageEventV1Fields;

    fn deref(&self) -> &RecordUsageEventV1Fields {
        &self.0
    }
}

impl TryFrom<RecordUsageEventV1Fields> for RecordUsageEventV1 {
    type Error = ContractError;

    fn try_from(fields: RecordUsageEventV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordUsageEventV1> for RecordUsageEventV1Fields {
    fn from(value: RecordUsageEventV1) -> RecordUsageEventV1Fields {
        value.0
    }
}

impl Serialize for RecordUsageEventV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:usage_event:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordUsageEventV1Version;

impl RecordUsageEventV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordUsageEventV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordUsageEventV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `role` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordUsageEventV1Role {
    #[serde(rename = "executor")]
    Executor,
    #[serde(rename = "reviewer")]
    Reviewer,
    #[serde(rename = "router")]
    Router,
    #[serde(rename = "advisor")]
    Advisor,
    #[serde(rename = "orchestrator")]
    Orchestrator,
}

impl RecordUsageEventV1Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
            Self::Router => "router",
            Self::Advisor => "advisor",
            Self::Orchestrator => "orchestrator",
        }
    }
}

/// `model_requested` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1ModelRequested(String);

impl RecordUsageEventV1ModelRequested {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUsageEventV1ModelRequested", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=128).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1ModelRequested {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1ModelRequested> for String {
    fn from(value: RecordUsageEventV1ModelRequested) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1ModelRequested {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1ModelRequested {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `model_effective` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1ModelEffective(String);

impl RecordUsageEventV1ModelEffective {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUsageEventV1ModelEffective", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=128).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1ModelEffective {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1ModelEffective> for String {
    fn from(value: RecordUsageEventV1ModelEffective) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1ModelEffective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1ModelEffective {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `effort_requested` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1EffortRequested(String);

impl RecordUsageEventV1EffortRequested {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1EffortRequested",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=32).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1EffortRequested {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1EffortRequested> for String {
    fn from(value: RecordUsageEventV1EffortRequested) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1EffortRequested {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1EffortRequested {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `effort_effective` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1EffortEffective(String);

impl RecordUsageEventV1EffortEffective {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1EffortEffective",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=32).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1EffortEffective {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1EffortEffective> for String {
    fn from(value: RecordUsageEventV1EffortEffective) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1EffortEffective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1EffortEffective {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `routing_mode` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordUsageEventV1RoutingMode {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "propose")]
    Propose,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "shadow")]
    Shadow,
}

impl RecordUsageEventV1RoutingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Propose => "propose",
            Self::Auto => "auto",
            Self::Shadow => "shadow",
        }
    }
}

/// `status` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordUsageEventV1Status {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "429")]
    V429,
    #[serde(rename = "aborted")]
    Aborted,
    #[serde(rename = "fallback")]
    Fallback,
}

impl RecordUsageEventV1Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::V429 => "429",
            Self::Aborted => "aborted",
            Self::Fallback => "fallback",
        }
    }
}

static RECORD_USAGE_EVENT_V1_ERROR_CODE_PATTERN: Pattern = Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `error_code` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1ErrorCode(String);

impl RecordUsageEventV1ErrorCode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUsageEventV1ErrorCode", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_USAGE_EVENT_V1_ERROR_CODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1ErrorCode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1ErrorCode> for String {
    fn from(value: RecordUsageEventV1ErrorCode) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `price_version` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1PriceVersion(String);

impl RecordUsageEventV1PriceVersion {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUsageEventV1PriceVersion", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=96).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1PriceVersion {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1PriceVersion> for String {
    fn from(value: RecordUsageEventV1PriceVersion) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1PriceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1PriceVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `cost_list_usd` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordUsageEventV1CostListUsd(Number);

impl RecordUsageEventV1CostListUsd {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value.as_f64().is_some_and(|number| number >= 0.0) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1CostListUsd",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordUsageEventV1CostListUsd {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordUsageEventV1CostListUsd> for Number {
    fn from(value: RecordUsageEventV1CostListUsd) -> Number {
        value.0
    }
}

impl Serialize for RecordUsageEventV1CostListUsd {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `cost_billable_usd` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordUsageEventV1CostBillableUsd(Number);

impl RecordUsageEventV1CostBillableUsd {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value.as_f64().is_some_and(|number| number >= 0.0) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1CostBillableUsd",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordUsageEventV1CostBillableUsd {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordUsageEventV1CostBillableUsd> for Number {
    fn from(value: RecordUsageEventV1CostBillableUsd) -> Number {
        value.0
    }
}

impl Serialize for RecordUsageEventV1CostBillableUsd {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `quota_window_before` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUsageEventV1QuotaWindowBefore {
    #[serde(deserialize_with = "support::nullable")]
    pub five_hour_pct: Option<RecordUsageEventV1QuotaWindowBeforeFiveHourPct>,
    #[serde(deserialize_with = "support::nullable")]
    pub weekly_pct: Option<RecordUsageEventV1QuotaWindowBeforeWeeklyPct>,
}

/// `quota_window_before.five_hour_pct` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordUsageEventV1QuotaWindowBeforeFiveHourPct(Number);

impl RecordUsageEventV1QuotaWindowBeforeFiveHourPct {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value
            .as_f64()
            .is_some_and(|number| (0.0..=100.0).contains(&number))
        {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1QuotaWindowBeforeFiveHourPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordUsageEventV1QuotaWindowBeforeFiveHourPct {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordUsageEventV1QuotaWindowBeforeFiveHourPct> for Number {
    fn from(value: RecordUsageEventV1QuotaWindowBeforeFiveHourPct) -> Number {
        value.0
    }
}

impl Serialize for RecordUsageEventV1QuotaWindowBeforeFiveHourPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `quota_window_before.weekly_pct` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordUsageEventV1QuotaWindowBeforeWeeklyPct(Number);

impl RecordUsageEventV1QuotaWindowBeforeWeeklyPct {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value
            .as_f64()
            .is_some_and(|number| (0.0..=100.0).contains(&number))
        {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1QuotaWindowBeforeWeeklyPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordUsageEventV1QuotaWindowBeforeWeeklyPct {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordUsageEventV1QuotaWindowBeforeWeeklyPct> for Number {
    fn from(value: RecordUsageEventV1QuotaWindowBeforeWeeklyPct) -> Number {
        value.0
    }
}

impl Serialize for RecordUsageEventV1QuotaWindowBeforeWeeklyPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `quota_window_after` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUsageEventV1QuotaWindowAfter {
    #[serde(deserialize_with = "support::nullable")]
    pub five_hour_pct: Option<RecordUsageEventV1QuotaWindowAfterFiveHourPct>,
    #[serde(deserialize_with = "support::nullable")]
    pub weekly_pct: Option<RecordUsageEventV1QuotaWindowAfterWeeklyPct>,
}

/// `quota_window_after.five_hour_pct` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordUsageEventV1QuotaWindowAfterFiveHourPct(Number);

impl RecordUsageEventV1QuotaWindowAfterFiveHourPct {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value
            .as_f64()
            .is_some_and(|number| (0.0..=100.0).contains(&number))
        {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1QuotaWindowAfterFiveHourPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordUsageEventV1QuotaWindowAfterFiveHourPct {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordUsageEventV1QuotaWindowAfterFiveHourPct> for Number {
    fn from(value: RecordUsageEventV1QuotaWindowAfterFiveHourPct) -> Number {
        value.0
    }
}

impl Serialize for RecordUsageEventV1QuotaWindowAfterFiveHourPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `quota_window_after.weekly_pct` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordUsageEventV1QuotaWindowAfterWeeklyPct(Number);

impl RecordUsageEventV1QuotaWindowAfterWeeklyPct {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value
            .as_f64()
            .is_some_and(|number| (0.0..=100.0).contains(&number))
        {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordUsageEventV1QuotaWindowAfterWeeklyPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordUsageEventV1QuotaWindowAfterWeeklyPct {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordUsageEventV1QuotaWindowAfterWeeklyPct> for Number {
    fn from(value: RecordUsageEventV1QuotaWindowAfterWeeklyPct) -> Number {
        value.0
    }
}

impl Serialize for RecordUsageEventV1QuotaWindowAfterWeeklyPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `source` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordUsageEventV1Source {
    #[serde(rename = "adapter")]
    Adapter,
    #[serde(rename = "cli_json")]
    CliJson,
    #[serde(rename = "statusline")]
    Statusline,
    #[serde(rename = "transcript")]
    Transcript,
    #[serde(rename = "rollout")]
    Rollout,
    #[serde(rename = "usagebar")]
    Usagebar,
    #[serde(rename = "manual")]
    Manual,
}

impl RecordUsageEventV1Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adapter => "adapter",
            Self::CliJson => "cli_json",
            Self::Statusline => "statusline",
            Self::Transcript => "transcript",
            Self::Rollout => "rollout",
            Self::Usagebar => "usagebar",
            Self::Manual => "manual",
        }
    }
}

/// `source_ref` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1SourceRef(String);

impl RecordUsageEventV1SourceRef {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUsageEventV1SourceRef", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=256).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1SourceRef {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1SourceRef> for String {
    fn from(value: RecordUsageEventV1SourceRef) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1SourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1SourceRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `dedupe_key` in `urn:ads:record:usage_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUsageEventV1DedupeKey(String);

impl RecordUsageEventV1DedupeKey {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUsageEventV1DedupeKey", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=256).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUsageEventV1DedupeKey {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUsageEventV1DedupeKey> for String {
    fn from(value: RecordUsageEventV1DedupeKey) -> String {
        value.0
    }
}

impl fmt::Display for RecordUsageEventV1DedupeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUsageEventV1DedupeKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
