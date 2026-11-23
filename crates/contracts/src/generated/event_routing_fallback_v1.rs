// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.routing.fallback.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Rota trocada por falha. A rota de uma tarefa caiu para o plano B depois de uma falha: registra de onde saiu, para onde foi e o código do erro que causou a troca. Nunca troca de conta.
///
/// `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV1 {
    pub id: CommonUlidV1,
    pub r#type: EventRoutingFallbackV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventRoutingFallbackV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventRoutingFallbackV1Data,
}

/// `type` in `urn:ads:event:routing.fallback:1`.
///
/// Always `"urn:ads:event:routing.fallback:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventRoutingFallbackV1Type;

impl EventRoutingFallbackV1Type {
    pub const VALUE: &str = "urn:ads:event:routing.fallback:1";
}

impl Serialize for EventRoutingFallbackV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventRoutingFallbackV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ROUTING_FALLBACK_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ROUTING_FALLBACK_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^routing/");

/// `stream` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV1Stream(String);

impl EventRoutingFallbackV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingFallbackV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_FALLBACK_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ROUTING_FALLBACK_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingFallbackV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV1Stream> for String {
    fn from(value: EventRoutingFallbackV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV1Data {
    pub task_id: CommonIdV1,
    pub from: EventRoutingFallbackV1DataFrom,
    pub to: EventRoutingFallbackV1DataTo,
    pub error_code: EventRoutingFallbackV1DataErrorCode,
}

/// `data.from` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV1DataFrom {
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingFallbackV1DataFromModel,
}

/// `data.from.model` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV1DataFromModel(String);

impl EventRoutingFallbackV1DataFromModel {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventRoutingFallbackV1DataFromModel",
                text,
            ))
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

impl TryFrom<String> for EventRoutingFallbackV1DataFromModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV1DataFromModel> for String {
    fn from(value: EventRoutingFallbackV1DataFromModel) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV1DataFromModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV1DataFromModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.to` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV1DataTo {
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingFallbackV1DataToModel,
}

/// `data.to.model` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV1DataToModel(String);

impl EventRoutingFallbackV1DataToModel {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventRoutingFallbackV1DataToModel",
                text,
            ))
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

impl TryFrom<String> for EventRoutingFallbackV1DataToModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV1DataToModel> for String {
    fn from(value: EventRoutingFallbackV1DataToModel) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV1DataToModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV1DataToModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static EVENT_ROUTING_FALLBACK_V1_DATA_ERROR_CODE_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `data.error_code` in `urn:ads:event:routing.fallback:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV1DataErrorCode(String);

impl EventRoutingFallbackV1DataErrorCode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventRoutingFallbackV1DataErrorCode",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_FALLBACK_V1_DATA_ERROR_CODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingFallbackV1DataErrorCode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV1DataErrorCode> for String {
    fn from(value: EventRoutingFallbackV1DataErrorCode) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV1DataErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV1DataErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
