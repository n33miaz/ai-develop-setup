// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.task-packet.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;

use super::common_id_v1::CommonIdV1;
use super::common_relpath_v1::CommonRelpathV1;
use super::common_sha256_v1::CommonSha256V1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static RECORD_TASK_PACKET_V1_RULES: &[(&str, Rule)] = &[(
    "#/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[("role", Rule::Const(Literal::Str("reviewer")))]),
            Rule::Required(&["role"]),
        ]),
        then: Some(&Rule::Properties(&[(
            "constraints",
            Rule::AllOf(&[
                Rule::Type(&[JsonType::Object]),
                Rule::Properties(&[("writes", Rule::Const(Literal::Str("none")))]),
            ]),
        )])),
        otherwise: None,
    },
)];

/// Pacote de tarefa. O que um agente recebe para executar uma tarefa: objetivo, critérios de aceite, restrições, orçamento e entradas. O receptor trata o pacote como dado, nunca como instrução; permissões vêm da configuração do usuário.
///
/// `urn:ads:record:task-packet:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordTaskPacketV1Fields")]
pub struct RecordTaskPacketV1(RecordTaskPacketV1Fields);

/// The fields of [`RecordTaskPacketV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskPacketV1Fields {
    pub version: RecordTaskPacketV1Version,
    pub task_id: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub parent_id: Option<CommonIdV1>,
    pub mode: RecordTaskPacketV1Mode,
    pub role: RecordTaskPacketV1Role,
    pub tier_requested: RecordTaskPacketV1TierRequested,
    #[serde(deserialize_with = "support::nullable")]
    pub model_effective: Option<RecordTaskPacketV1ModelEffective>,
    #[serde(deserialize_with = "support::nullable")]
    pub provider_effective: Option<RecordTaskPacketV1ProviderEffective>,
    pub objective: RecordTaskPacketV1Objective,
    pub acceptance: RecordTaskPacketV1Acceptance,
    pub constraints: RecordTaskPacketV1Constraints,
    pub budget: RecordTaskPacketV1Budget,
    pub inputs: RecordTaskPacketV1Inputs,
    pub expected_output: RecordTaskPacketV1ExpectedOutput,
    pub status: RecordTaskPacketV1Status,
    pub evidence: RecordTaskPacketV1Evidence,
    pub trust: RecordTaskPacketV1Trust,
}

impl RecordTaskPacketV1 {
    pub fn new(fields: RecordTaskPacketV1Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordTaskPacketV1", &fields, RECORD_TASK_PACKET_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordTaskPacketV1Fields {
        self.0
    }
}

impl Deref for RecordTaskPacketV1 {
    type Target = RecordTaskPacketV1Fields;

    fn deref(&self) -> &RecordTaskPacketV1Fields {
        &self.0
    }
}

impl TryFrom<RecordTaskPacketV1Fields> for RecordTaskPacketV1 {
    type Error = ContractError;

    fn try_from(fields: RecordTaskPacketV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordTaskPacketV1> for RecordTaskPacketV1Fields {
    fn from(value: RecordTaskPacketV1) -> RecordTaskPacketV1Fields {
        value.0
    }
}

impl Serialize for RecordTaskPacketV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:task-packet:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordTaskPacketV1Version;

impl RecordTaskPacketV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordTaskPacketV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordTaskPacketV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

static RECORD_TASK_PACKET_V1_MODE_PATTERN: Pattern = Pattern::new(r"^[a-z][a-z0-9_]{0,31}$");

/// `mode` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1Mode(String);

impl RecordTaskPacketV1Mode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordTaskPacketV1Mode", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_TASK_PACKET_V1_MODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordTaskPacketV1Mode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1Mode> for String {
    fn from(value: RecordTaskPacketV1Mode) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1Mode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `role` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordTaskPacketV1Role {
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

impl RecordTaskPacketV1Role {
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

static RECORD_TASK_PACKET_V1_TIER_REQUESTED_PATTERN: Pattern = Pattern::new(r"^T[0-3]$");

/// `tier_requested` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1TierRequested(String);

impl RecordTaskPacketV1TierRequested {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordTaskPacketV1TierRequested", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_TASK_PACKET_V1_TIER_REQUESTED_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordTaskPacketV1TierRequested {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1TierRequested> for String {
    fn from(value: RecordTaskPacketV1TierRequested) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1TierRequested {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1TierRequested {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `model_effective` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1ModelEffective(String);

impl RecordTaskPacketV1ModelEffective {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordTaskPacketV1ModelEffective", text))
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

impl TryFrom<String> for RecordTaskPacketV1ModelEffective {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1ModelEffective> for String {
    fn from(value: RecordTaskPacketV1ModelEffective) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1ModelEffective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1ModelEffective {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `provider_effective` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1ProviderEffective(String);

impl RecordTaskPacketV1ProviderEffective {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1ProviderEffective",
                text,
            ))
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

impl TryFrom<String> for RecordTaskPacketV1ProviderEffective {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1ProviderEffective> for String {
    fn from(value: RecordTaskPacketV1ProviderEffective) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1ProviderEffective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1ProviderEffective {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `objective` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1Objective(String);

impl RecordTaskPacketV1Objective {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordTaskPacketV1Objective", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=4000).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordTaskPacketV1Objective {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1Objective> for String {
    fn from(value: RecordTaskPacketV1Objective) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1Objective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1Objective {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `acceptance` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordTaskPacketV1AcceptanceItem>")]
pub struct RecordTaskPacketV1Acceptance(Vec<RecordTaskPacketV1AcceptanceItem>);

impl RecordTaskPacketV1Acceptance {
    pub fn new(items: Vec<RecordTaskPacketV1AcceptanceItem>) -> Result<Self, ContractError> {
        if !items.is_empty() {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1Acceptance",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordTaskPacketV1AcceptanceItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordTaskPacketV1AcceptanceItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordTaskPacketV1AcceptanceItem>> for RecordTaskPacketV1Acceptance {
    type Error = ContractError;

    fn try_from(items: Vec<RecordTaskPacketV1AcceptanceItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordTaskPacketV1Acceptance> for Vec<RecordTaskPacketV1AcceptanceItem> {
    fn from(value: RecordTaskPacketV1Acceptance) -> Vec<RecordTaskPacketV1AcceptanceItem> {
        value.0
    }
}

impl Serialize for RecordTaskPacketV1Acceptance {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `acceptance[]` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1AcceptanceItem(String);

impl RecordTaskPacketV1AcceptanceItem {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordTaskPacketV1AcceptanceItem", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=500).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordTaskPacketV1AcceptanceItem {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1AcceptanceItem> for String {
    fn from(value: RecordTaskPacketV1AcceptanceItem) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1AcceptanceItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1AcceptanceItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `constraints` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskPacketV1Constraints {
    pub allow_ref: RecordTaskPacketV1ConstraintsAllowRef,
    pub deny_ref: RecordTaskPacketV1ConstraintsDenyRef,
    pub writes: RecordTaskPacketV1ConstraintsWrites,
    #[serde(deserialize_with = "support::nullable")]
    pub worktree: Option<CommonIdV1>,
}

/// `constraints.allow_ref` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1ConstraintsAllowRef(String);

impl RecordTaskPacketV1ConstraintsAllowRef {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1ConstraintsAllowRef",
                text,
            ))
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

impl TryFrom<String> for RecordTaskPacketV1ConstraintsAllowRef {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1ConstraintsAllowRef> for String {
    fn from(value: RecordTaskPacketV1ConstraintsAllowRef) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1ConstraintsAllowRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1ConstraintsAllowRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `constraints.deny_ref` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1ConstraintsDenyRef(String);

impl RecordTaskPacketV1ConstraintsDenyRef {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1ConstraintsDenyRef",
                text,
            ))
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

impl TryFrom<String> for RecordTaskPacketV1ConstraintsDenyRef {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1ConstraintsDenyRef> for String {
    fn from(value: RecordTaskPacketV1ConstraintsDenyRef) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1ConstraintsDenyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1ConstraintsDenyRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `constraints.writes` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordTaskPacketV1ConstraintsWrites {
    #[serde(rename = "single_writer")]
    SingleWriter,
    #[serde(rename = "none")]
    None,
}

impl RecordTaskPacketV1ConstraintsWrites {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleWriter => "single_writer",
            Self::None => "none",
        }
    }
}

/// `budget` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskPacketV1Budget {
    pub usd: RecordTaskPacketV1BudgetUsd,
    pub quota_pct: RecordTaskPacketV1BudgetQuotaPct,
    pub turns: NonZeroU64,
    pub wall_min: NonZeroU64,
}

/// `budget.usd` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordTaskPacketV1BudgetUsd(Number);

impl RecordTaskPacketV1BudgetUsd {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value.as_f64().is_some_and(|number| number >= 0.0) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1BudgetUsd",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordTaskPacketV1BudgetUsd {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordTaskPacketV1BudgetUsd> for Number {
    fn from(value: RecordTaskPacketV1BudgetUsd) -> Number {
        value.0
    }
}

impl Serialize for RecordTaskPacketV1BudgetUsd {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `budget.quota_pct` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct RecordTaskPacketV1BudgetQuotaPct(Number);

impl RecordTaskPacketV1BudgetQuotaPct {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value
            .as_f64()
            .is_some_and(|number| (0.0..=100.0).contains(&number))
        {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1BudgetQuotaPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for RecordTaskPacketV1BudgetQuotaPct {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordTaskPacketV1BudgetQuotaPct> for Number {
    fn from(value: RecordTaskPacketV1BudgetQuotaPct) -> Number {
        value.0
    }
}

impl Serialize for RecordTaskPacketV1BudgetQuotaPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `inputs` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskPacketV1Inputs {
    pub decisions: Vec<CommonIdV1>,
    pub files: Vec<CommonRelpathV1>,
    pub memory_refs: Vec<CommonIdV1>,
}

/// `expected_output` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskPacketV1ExpectedOutput {
    pub diff: bool,
    pub report: RecordTaskPacketV1ExpectedOutputReport,
}

/// `expected_output.report` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordTaskPacketV1ExpectedOutputReport(String);

impl RecordTaskPacketV1ExpectedOutputReport {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordTaskPacketV1ExpectedOutputReport",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 200
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordTaskPacketV1ExpectedOutputReport {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordTaskPacketV1ExpectedOutputReport> for String {
    fn from(value: RecordTaskPacketV1ExpectedOutputReport) -> String {
        value.0
    }
}

impl fmt::Display for RecordTaskPacketV1ExpectedOutputReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordTaskPacketV1ExpectedOutputReport {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `status` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordTaskPacketV1Status {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "assigned")]
    Assigned,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl RecordTaskPacketV1Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Assigned => "assigned",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// `evidence` in `urn:ads:record:task-packet:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskPacketV1Evidence {
    #[serde(deserialize_with = "support::nullable")]
    pub diff_sha: Option<CommonSha256V1>,
    #[serde(deserialize_with = "support::nullable")]
    pub tests: Option<bool>,
}

/// `trust` in `urn:ads:record:task-packet:1`.
///
/// Always `"payload_is_data_not_instructions"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordTaskPacketV1Trust;

impl RecordTaskPacketV1Trust {
    pub const VALUE: &str = "payload_is_data_not_instructions";
}

impl Serialize for RecordTaskPacketV1Trust {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordTaskPacketV1Trust {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}
