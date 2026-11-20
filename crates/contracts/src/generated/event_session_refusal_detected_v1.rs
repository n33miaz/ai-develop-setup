// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.refusal_detected.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Recusa ou filtro detectado. A ferramenta recusou um pedido ou um filtro de segurança interrompeu a sessão. Só metadados: ferramenta, provedor, modelo e tipo. O texto do pedido e da resposta não cabe no evento.
///
/// `urn:ads:event:session.refusal_detected:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionRefusalDetectedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionRefusalDetectedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionRefusalDetectedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionRefusalDetectedV1DataClass,
    pub data: EventSessionRefusalDetectedV1Data,
}

/// `type` in `urn:ads:event:session.refusal_detected:1`.
///
/// Always `"urn:ads:event:session.refusal_detected:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionRefusalDetectedV1Type;

impl EventSessionRefusalDetectedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.refusal_detected:1";
}

impl Serialize for EventSessionRefusalDetectedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionRefusalDetectedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_REFUSAL_DETECTED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_REFUSAL_DETECTED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.refusal_detected:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionRefusalDetectedV1Stream(String);

impl EventSessionRefusalDetectedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionRefusalDetectedV1Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_REFUSAL_DETECTED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_REFUSAL_DETECTED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionRefusalDetectedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionRefusalDetectedV1Stream> for String {
    fn from(value: EventSessionRefusalDetectedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionRefusalDetectedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionRefusalDetectedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.refusal_detected:1`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionRefusalDetectedV1DataClass;

impl EventSessionRefusalDetectedV1DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionRefusalDetectedV1DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionRefusalDetectedV1DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:session.refusal_detected:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionRefusalDetectedV1Data {
    pub session_id: CommonIdV1,
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventSessionRefusalDetectedV1DataModel,
    pub kind: EventSessionRefusalDetectedV1DataKind,
}

/// `data.model` in `urn:ads:event:session.refusal_detected:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionRefusalDetectedV1DataModel(String);

impl EventSessionRefusalDetectedV1DataModel {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionRefusalDetectedV1DataModel",
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

impl TryFrom<String> for EventSessionRefusalDetectedV1DataModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionRefusalDetectedV1DataModel> for String {
    fn from(value: EventSessionRefusalDetectedV1DataModel) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionRefusalDetectedV1DataModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionRefusalDetectedV1DataModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.kind` in `urn:ads:event:session.refusal_detected:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionRefusalDetectedV1DataKind {
    #[serde(rename = "refusal")]
    Refusal,
    #[serde(rename = "safety_filter")]
    SafetyFilter,
}

impl EventSessionRefusalDetectedV1DataKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Refusal => "refusal",
            Self::SafetyFilter => "safety_filter",
        }
    }
}
