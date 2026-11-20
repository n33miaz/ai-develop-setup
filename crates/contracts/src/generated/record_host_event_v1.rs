// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.host_event.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::ContractError;

/// Alerta do host. Um alerta derivado de amostras: cita a regra, a gravidade e os sample_id que o justificam. Confirmar é um evento posterior; o alerta original não muda.
///
/// `urn:ads:record:host_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordHostEventV1 {
    pub version: RecordHostEventV1Version,
    pub event_id: CommonIdV1,
    pub rule_id: CommonIdV1,
    pub severity: RecordHostEventV1Severity,
    pub signals: RecordHostEventV1Signals,
    pub why: RecordHostEventV1Why,
    pub ts: CommonTimestampV1,
    pub ack: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub task_id: Option<Option<CommonIdV1>>,
}

/// `version` in `urn:ads:record:host_event:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordHostEventV1Version;

impl RecordHostEventV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordHostEventV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordHostEventV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `severity` in `urn:ads:record:host_event:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordHostEventV1Severity {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "critical")]
    Critical,
}

impl RecordHostEventV1Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Critical => "critical",
        }
    }
}

/// `signals` in `urn:ads:record:host_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct RecordHostEventV1Signals(Vec<CommonIdV1>);

impl RecordHostEventV1Signals {
    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if !items.is_empty() && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordHostEventV1Signals",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommonIdV1] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommonIdV1> {
        self.0
    }
}

impl TryFrom<Vec<CommonIdV1>> for RecordHostEventV1Signals {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordHostEventV1Signals> for Vec<CommonIdV1> {
    fn from(value: RecordHostEventV1Signals) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for RecordHostEventV1Signals {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `why` in `urn:ads:record:host_event:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordHostEventV1Why(String);

impl RecordHostEventV1Why {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordHostEventV1Why", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordHostEventV1Why {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordHostEventV1Why> for String {
    fn from(value: RecordHostEventV1Why) -> String {
        value.0
    }
}

impl fmt::Display for RecordHostEventV1Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordHostEventV1Why {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
