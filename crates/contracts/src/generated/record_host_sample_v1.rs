// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.host_sample.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, JsonType, Pattern, Rule};

static RECORD_HOST_SAMPLE_V1_RULE_PATTERN_1: Pattern = Pattern::new(r"^host\.disk\.free_(gb|pct)$");

static RECORD_HOST_SAMPLE_V1_RULES: &[(&str, Rule)] = &[(
    "#/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[(
                "signal_id",
                Rule::AllOf(&[
                    Rule::Type(&[JsonType::String]),
                    Rule::Pattern(&RECORD_HOST_SAMPLE_V1_RULE_PATTERN_1),
                ]),
            )]),
            Rule::Required(&["signal_id"]),
        ]),
        then: Some(&Rule::Properties(&[(
            "value",
            Rule::AllOf(&[Rule::Type(&[JsonType::Number]), Rule::Minimum(0.0)]),
        )])),
        otherwise: None,
    },
)];

/// Amostra do host. Uma observação de um sinal da máquina (disco, memória, etc.) feita por um coletor só leitura. Amostra ausente nunca vira zero; corrigir é gravar outra amostra com outro sample_id.
///
/// `urn:ads:record:host_sample:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordHostSampleV1Fields")]
pub struct RecordHostSampleV1(RecordHostSampleV1Fields);

/// The fields of [`RecordHostSampleV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordHostSampleV1Fields {
    pub version: RecordHostSampleV1Version,
    pub sample_id: CommonIdV1,
    pub signal_id: RecordHostSampleV1SignalId,
    pub value: RecordHostSampleV1Value,
    pub unit: RecordHostSampleV1Unit,
    pub ts: CommonTimestampV1,
    pub collector: CommonIdV1,
    pub cost_ms: u64,
    pub scope_alias: CommonIdV1,
    pub stale: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub task_id: Option<Option<CommonIdV1>>,
}

impl RecordHostSampleV1 {
    pub fn new(fields: RecordHostSampleV1Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordHostSampleV1", &fields, RECORD_HOST_SAMPLE_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordHostSampleV1Fields {
        self.0
    }
}

impl Deref for RecordHostSampleV1 {
    type Target = RecordHostSampleV1Fields;

    fn deref(&self) -> &RecordHostSampleV1Fields {
        &self.0
    }
}

impl TryFrom<RecordHostSampleV1Fields> for RecordHostSampleV1 {
    type Error = ContractError;

    fn try_from(fields: RecordHostSampleV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordHostSampleV1> for RecordHostSampleV1Fields {
    fn from(value: RecordHostSampleV1) -> RecordHostSampleV1Fields {
        value.0
    }
}

impl Serialize for RecordHostSampleV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:host_sample:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordHostSampleV1Version;

impl RecordHostSampleV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordHostSampleV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordHostSampleV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

static RECORD_HOST_SAMPLE_V1_SIGNAL_ID_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$");

/// `signal_id` in `urn:ads:record:host_sample:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordHostSampleV1SignalId(String);

impl RecordHostSampleV1SignalId {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordHostSampleV1SignalId", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 96 && RECORD_HOST_SAMPLE_V1_SIGNAL_ID_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordHostSampleV1SignalId {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordHostSampleV1SignalId> for String {
    fn from(value: RecordHostSampleV1SignalId) -> String {
        value.0
    }
}

impl fmt::Display for RecordHostSampleV1SignalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordHostSampleV1SignalId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `value` in `urn:ads:record:host_sample:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RecordHostSampleV1Value {
    Number(Number),
    Boolean(bool),
    String(String),
}

/// `unit` in `urn:ads:record:host_sample:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordHostSampleV1Unit(String);

impl RecordHostSampleV1Unit {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordHostSampleV1Unit", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=16).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordHostSampleV1Unit {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordHostSampleV1Unit> for String {
    fn from(value: RecordHostSampleV1Unit) -> String {
        value.0
    }
}

impl fmt::Display for RecordHostSampleV1Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordHostSampleV1Unit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
