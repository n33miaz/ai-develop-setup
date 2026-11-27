// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.api_usage.v3.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, Literal, Rule};

static RECORD_API_USAGE_V3_RULES: &[(&str, Rule)] = &[(
    "#/if",
    Rule::If {
        when: &Rule::Properties(&[("outcome", Rule::Const(Literal::Str("succeeded_reported")))]),
        then: Some(&Rule::Properties(&[(
            "cost_basis",
            Rule::Const(Literal::Str("reported")),
        )])),
        otherwise: Some(&Rule::If {
            when: &Rule::Properties(&[(
                "outcome",
                Rule::Enum(&[
                    Literal::Str("succeeded_reserved"),
                    Literal::Str("charged_uncertain"),
                    Literal::Str("charged_error_status"),
                    Literal::Str("charged_bad_response"),
                ]),
            )]),
            then: Some(&Rule::Properties(&[(
                "cost_basis",
                Rule::Const(Literal::Str("reserved")),
            )])),
            otherwise: None,
        }),
    },
)];

/// Uso de uma chamada de API (com desfecho). O que o adaptador de API informa de uma chamada direta a um provedor: alias do provedor, modelo, tokens, custo estimado em micro-dólares, base do custo e desfecho. Nunca leva texto do pedido nem da resposta, nem chave. O chamador o amplia para o registro usage_event. As contagens e o custo usam inteiros de 0 a 9007199254740991 (2^53 - 1), representáveis exatamente em Rust e TypeScript.
///
/// `urn:ads:record:api_usage:3`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordApiUsageV3Fields")]
pub struct RecordApiUsageV3(RecordApiUsageV3Fields);

/// The fields of [`RecordApiUsageV3`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordApiUsageV3Fields {
    pub version: RecordApiUsageV3Version,
    pub ts: CommonTimestampV1,
    pub provider_alias: CommonIdV1,
    pub model: RecordApiUsageV3Model,
    pub tokens_in: RecordApiUsageV3SafeInteger,
    pub tokens_out: RecordApiUsageV3SafeInteger,
    pub cost_estimate_micro_usd: RecordApiUsageV3SafeInteger,
    pub cost_basis: RecordApiUsageV3CostBasis,
    pub price_version: RecordApiUsageV3PriceVersion,
    pub outcome: RecordApiUsageV3Outcome,
}

impl RecordApiUsageV3 {
    pub fn new(fields: RecordApiUsageV3Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordApiUsageV3", &fields, RECORD_API_USAGE_V3_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordApiUsageV3Fields {
        self.0
    }
}

impl Deref for RecordApiUsageV3 {
    type Target = RecordApiUsageV3Fields;

    fn deref(&self) -> &RecordApiUsageV3Fields {
        &self.0
    }
}

impl TryFrom<RecordApiUsageV3Fields> for RecordApiUsageV3 {
    type Error = ContractError;

    fn try_from(fields: RecordApiUsageV3Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordApiUsageV3> for RecordApiUsageV3Fields {
    fn from(value: RecordApiUsageV3) -> RecordApiUsageV3Fields {
        value.0
    }
}

impl Serialize for RecordApiUsageV3 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:api_usage:3`.
///
/// Always `3`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordApiUsageV3Version;

impl RecordApiUsageV3Version {
    pub const VALUE: i64 = 3;
}

impl Serialize for RecordApiUsageV3Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordApiUsageV3Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `model` in `urn:ads:record:api_usage:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApiUsageV3Model(String);

impl RecordApiUsageV3Model {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApiUsageV3Model", text))
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

impl TryFrom<String> for RecordApiUsageV3Model {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApiUsageV3Model> for String {
    fn from(value: RecordApiUsageV3Model) -> String {
        value.0
    }
}

impl fmt::Display for RecordApiUsageV3Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApiUsageV3Model {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `$defs.safe_integer` in `urn:ads:record:api_usage:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordApiUsageV3SafeInteger(u64);

impl RecordApiUsageV3SafeInteger {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordApiUsageV3SafeInteger",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordApiUsageV3SafeInteger {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordApiUsageV3SafeInteger> for u64 {
    fn from(value: RecordApiUsageV3SafeInteger) -> u64 {
        value.0
    }
}

impl Serialize for RecordApiUsageV3SafeInteger {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `cost_basis` in `urn:ads:record:api_usage:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApiUsageV3CostBasis {
    #[serde(rename = "reported")]
    Reported,
    #[serde(rename = "reserved")]
    Reserved,
}

impl RecordApiUsageV3CostBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reported => "reported",
            Self::Reserved => "reserved",
        }
    }
}

/// `price_version` in `urn:ads:record:api_usage:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApiUsageV3PriceVersion(String);

impl RecordApiUsageV3PriceVersion {
    pub const MAX_CHARS: usize = 96;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApiUsageV3PriceVersion", text))
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

impl TryFrom<String> for RecordApiUsageV3PriceVersion {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApiUsageV3PriceVersion> for String {
    fn from(value: RecordApiUsageV3PriceVersion) -> String {
        value.0
    }
}

impl fmt::Display for RecordApiUsageV3PriceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApiUsageV3PriceVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `outcome` in `urn:ads:record:api_usage:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApiUsageV3Outcome {
    #[serde(rename = "succeeded_reported")]
    SucceededReported,
    #[serde(rename = "succeeded_reserved")]
    SucceededReserved,
    #[serde(rename = "charged_uncertain")]
    ChargedUncertain,
    #[serde(rename = "charged_error_status")]
    ChargedErrorStatus,
    #[serde(rename = "charged_bad_response")]
    ChargedBadResponse,
    #[serde(rename = "charged_guard_replaced")]
    ChargedGuardReplaced,
}

impl RecordApiUsageV3Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SucceededReported => "succeeded_reported",
            Self::SucceededReserved => "succeeded_reserved",
            Self::ChargedUncertain => "charged_uncertain",
            Self::ChargedErrorStatus => "charged_error_status",
            Self::ChargedBadResponse => "charged_bad_response",
            Self::ChargedGuardReplaced => "charged_guard_replaced",
        }
    }
}
