// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.envelope.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Envelope de evento. Envelope único de todo evento: fato gravado só-anexo, numerado por stream, com autor, causa e classe do dado. Cada tipo de evento estende este envelope e fixa type, stream e data.
///
/// `urn:ads:event:envelope:1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelopeV1 {
    pub id: CommonUlidV1,
    pub r#type: EventEnvelopeV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventEnvelopeV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: Map<String, Value>,
}

static EVENT_ENVELOPE_V1_TYPE_PATTERN: Pattern =
    Pattern::new(r"^urn:ads:event:[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+:[1-9][0-9]*$");

/// `type` in `urn:ads:event:envelope:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventEnvelopeV1Type(String);

impl EventEnvelopeV1Type {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventEnvelopeV1Type", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ENVELOPE_V1_TYPE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventEnvelopeV1Type {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventEnvelopeV1Type> for String {
    fn from(value: EventEnvelopeV1Type) -> String {
        value.0
    }
}

impl fmt::Display for EventEnvelopeV1Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventEnvelopeV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static EVENT_ENVELOPE_V1_STREAM_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");

/// `stream` in `urn:ads:event:envelope:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventEnvelopeV1Stream(String);

impl EventEnvelopeV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventEnvelopeV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ENVELOPE_V1_STREAM_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventEnvelopeV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventEnvelopeV1Stream> for String {
    fn from(value: EventEnvelopeV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventEnvelopeV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventEnvelopeV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
