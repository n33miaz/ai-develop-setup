// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.api_usage.v2.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, Literal, Rule};

static RECORD_API_USAGE_V2_RULES: &[(&str, Rule)] = &[(
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

/// Uso de uma chamada de API (com desfecho). O que o adaptador de API informa de uma chamada direta a um provedor: alias do provedor, modelo, tokens, custo estimado em micro-dólares, base do custo e desfecho. Nunca leva texto do pedido nem da resposta, nem chave. O chamador o amplia para o registro usage_event.
///
/// `urn:ads:record:api_usage:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordApiUsageV2Fields")]
pub struct RecordApiUsageV2(RecordApiUsageV2Fields);

/// The fields of [`RecordApiUsageV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordApiUsageV2Fields {
    pub version: RecordApiUsageV2Version,
    pub ts: CommonTimestampV1,
    pub provider_alias: CommonIdV1,
    pub model: RecordApiUsageV2Model,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_estimate_micro_usd: u64,
    pub cost_basis: RecordApiUsageV2CostBasis,
    pub price_version: RecordApiUsageV2PriceVersion,
    pub outcome: RecordApiUsageV2Outcome,
}

impl RecordApiUsageV2 {
    pub fn new(fields: RecordApiUsageV2Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordApiUsageV2", &fields, RECORD_API_USAGE_V2_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordApiUsageV2Fields {
        self.0
    }
}

impl Deref for RecordApiUsageV2 {
    type Target = RecordApiUsageV2Fields;

    fn deref(&self) -> &RecordApiUsageV2Fields {
        &self.0
    }
}

impl TryFrom<RecordApiUsageV2Fields> for RecordApiUsageV2 {
    type Error = ContractError;

    fn try_from(fields: RecordApiUsageV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordApiUsageV2> for RecordApiUsageV2Fields {
    fn from(value: RecordApiUsageV2) -> RecordApiUsageV2Fields {
        value.0
    }
}

impl Serialize for RecordApiUsageV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:api_usage:2`.
///
/// Always `2`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordApiUsageV2Version;

impl RecordApiUsageV2Version {
    pub const VALUE: i64 = 2;
}

impl Serialize for RecordApiUsageV2Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordApiUsageV2Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `model` in `urn:ads:record:api_usage:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApiUsageV2Model(String);

impl RecordApiUsageV2Model {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApiUsageV2Model", text))
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

impl TryFrom<String> for RecordApiUsageV2Model {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApiUsageV2Model> for String {
    fn from(value: RecordApiUsageV2Model) -> String {
        value.0
    }
}

impl fmt::Display for RecordApiUsageV2Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApiUsageV2Model {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `cost_basis` in `urn:ads:record:api_usage:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApiUsageV2CostBasis {
    #[serde(rename = "reported")]
    Reported,
    #[serde(rename = "reserved")]
    Reserved,
}

impl RecordApiUsageV2CostBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reported => "reported",
            Self::Reserved => "reserved",
        }
    }
}

/// `price_version` in `urn:ads:record:api_usage:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApiUsageV2PriceVersion(String);

impl RecordApiUsageV2PriceVersion {
    pub const MAX_CHARS: usize = 96;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApiUsageV2PriceVersion", text))
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

impl TryFrom<String> for RecordApiUsageV2PriceVersion {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApiUsageV2PriceVersion> for String {
    fn from(value: RecordApiUsageV2PriceVersion) -> String {
        value.0
    }
}

impl fmt::Display for RecordApiUsageV2PriceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApiUsageV2PriceVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `outcome` in `urn:ads:record:api_usage:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApiUsageV2Outcome {
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

impl RecordApiUsageV2Outcome {
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
