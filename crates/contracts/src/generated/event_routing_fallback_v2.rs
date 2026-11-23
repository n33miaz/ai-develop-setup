// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.routing.fallback.v2.schema.json. Não edite à mão.

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

/// Rota trocada por falha. A rota caiu para o plano B após uma falha. Registra também os apelidos de conta das rotas, quando aplicáveis.
///
/// `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV2 {
    pub id: CommonUlidV1,
    pub r#type: EventRoutingFallbackV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventRoutingFallbackV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventRoutingFallbackV2Data,
}

/// `type` in `urn:ads:event:routing.fallback:2`.
///
/// Always `"urn:ads:event:routing.fallback:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventRoutingFallbackV2Type;

impl EventRoutingFallbackV2Type {
    pub const VALUE: &str = "urn:ads:event:routing.fallback:2";
}

impl Serialize for EventRoutingFallbackV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventRoutingFallbackV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ROUTING_FALLBACK_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ROUTING_FALLBACK_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^routing/");

/// `stream` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV2Stream(String);

impl EventRoutingFallbackV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingFallbackV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_FALLBACK_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_ROUTING_FALLBACK_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingFallbackV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV2Stream> for String {
    fn from(value: EventRoutingFallbackV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV2Data {
    pub task_id: CommonIdV1,
    pub from: EventRoutingFallbackV2DataFrom,
    pub to: EventRoutingFallbackV2DataTo,
    pub error_code: EventRoutingFallbackV2DataErrorCode,
}

/// `data.from` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV2DataFrom {
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingFallbackV2DataFromModel,
    #[serde(deserialize_with = "support::nullable")]
    pub account_alias: Option<CommonIdV1>,
}

/// `data.from.model` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV2DataFromModel(String);

impl EventRoutingFallbackV2DataFromModel {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventRoutingFallbackV2DataFromModel",
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

impl TryFrom<String> for EventRoutingFallbackV2DataFromModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV2DataFromModel> for String {
    fn from(value: EventRoutingFallbackV2DataFromModel) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV2DataFromModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV2DataFromModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.to` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingFallbackV2DataTo {
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingFallbackV2DataToModel,
    #[serde(deserialize_with = "support::nullable")]
    pub account_alias: Option<CommonIdV1>,
}

/// `data.to.model` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV2DataToModel(String);

impl EventRoutingFallbackV2DataToModel {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventRoutingFallbackV2DataToModel",
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

impl TryFrom<String> for EventRoutingFallbackV2DataToModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV2DataToModel> for String {
    fn from(value: EventRoutingFallbackV2DataToModel) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV2DataToModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV2DataToModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static EVENT_ROUTING_FALLBACK_V2_DATA_ERROR_CODE_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `data.error_code` in `urn:ads:event:routing.fallback:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingFallbackV2DataErrorCode(String);

impl EventRoutingFallbackV2DataErrorCode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventRoutingFallbackV2DataErrorCode",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_FALLBACK_V2_DATA_ERROR_CODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingFallbackV2DataErrorCode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingFallbackV2DataErrorCode> for String {
    fn from(value: EventRoutingFallbackV2DataErrorCode) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingFallbackV2DataErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingFallbackV2DataErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
