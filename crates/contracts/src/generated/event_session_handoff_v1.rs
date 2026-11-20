// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.handoff.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Decisão de passagem entre sessões. Registra a proposta ou a decisão sobre uma passagem. O registro da passagem autorizada usa o mesmo identificador.
///
/// `urn:ads:event:session.handoff:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV1 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionHandoffV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionHandoffV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionHandoffV1DataClass,
    pub data: EventSessionHandoffV1Data,
}

/// `type` in `urn:ads:event:session.handoff:1`.
///
/// Always `"urn:ads:event:session.handoff:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV1Type;

impl EventSessionHandoffV1Type {
    pub const VALUE: &str = "urn:ads:event:session.handoff:1";
}

impl Serialize for EventSessionHandoffV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_HANDOFF_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^handoff/");

/// `stream` in `urn:ads:event:session.handoff:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionHandoffV1Stream(String);

impl EventSessionHandoffV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionHandoffV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_HANDOFF_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_HANDOFF_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionHandoffV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionHandoffV1Stream> for String {
    fn from(value: EventSessionHandoffV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionHandoffV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionHandoffV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.handoff:1`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV1DataClass;

impl EventSessionHandoffV1DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionHandoffV1DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV1DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:session.handoff:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV1Data {
    pub handoff_id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    pub status: EventSessionHandoffV1DataStatus,
}

/// `data.status` in `urn:ads:event:session.handoff:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionHandoffV1DataStatus {
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "declined")]
    Declined,
}

impl EventSessionHandoffV1DataStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }
}
