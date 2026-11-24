// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.api_usage.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::ContractError;

/// Uso de uma chamada de API. O que o adaptador de API informa de uma chamada direta a um provedor: alias do provedor, modelo, tokens, custo estimado em micro-dólares e a base do custo. Nunca leva texto do pedido nem da resposta, nem chave. O chamador o amplia para o registro usage_event.
///
/// `urn:ads:record:api_usage:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordApiUsageV1 {
    pub version: RecordApiUsageV1Version,
    pub ts: CommonTimestampV1,
    pub provider_alias: CommonIdV1,
    pub model: RecordApiUsageV1Model,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_estimate_micro_usd: u64,
    pub cost_basis: RecordApiUsageV1CostBasis,
    pub price_version: RecordApiUsageV1PriceVersion,
}

/// `version` in `urn:ads:record:api_usage:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordApiUsageV1Version;

impl RecordApiUsageV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordApiUsageV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordApiUsageV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `model` in `urn:ads:record:api_usage:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApiUsageV1Model(String);

impl RecordApiUsageV1Model {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApiUsageV1Model", text))
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

impl TryFrom<String> for RecordApiUsageV1Model {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApiUsageV1Model> for String {
    fn from(value: RecordApiUsageV1Model) -> String {
        value.0
    }
}

impl fmt::Display for RecordApiUsageV1Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApiUsageV1Model {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `cost_basis` in `urn:ads:record:api_usage:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApiUsageV1CostBasis {
    #[serde(rename = "reported")]
    Reported,
    #[serde(rename = "reserved")]
    Reserved,
}

impl RecordApiUsageV1CostBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reported => "reported",
            Self::Reserved => "reserved",
        }
    }
}

/// `price_version` in `urn:ads:record:api_usage:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApiUsageV1PriceVersion(String);

impl RecordApiUsageV1PriceVersion {
    pub const MAX_CHARS: usize = 96;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApiUsageV1PriceVersion", text))
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

impl TryFrom<String> for RecordApiUsageV1PriceVersion {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApiUsageV1PriceVersion> for String {
    fn from(value: RecordApiUsageV1PriceVersion) -> String {
        value.0
    }
}

impl fmt::Display for RecordApiUsageV1PriceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApiUsageV1PriceVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
