// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.resized.v1.schema.json. Não edite à mão.

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

/// Sessão redimensionada. O terminal da sessão mudou de tamanho.
///
/// `urn:ads:event:session.resized:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionResizedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionResizedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionResizedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventSessionResizedV1Data,
}

/// `type` in `urn:ads:event:session.resized:1`.
///
/// Always `"urn:ads:event:session.resized:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionResizedV1Type;

impl EventSessionResizedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.resized:1";
}

impl Serialize for EventSessionResizedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionResizedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_RESIZED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_RESIZED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.resized:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionResizedV1Stream(String);

impl EventSessionResizedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionResizedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_RESIZED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_RESIZED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionResizedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionResizedV1Stream> for String {
    fn from(value: EventSessionResizedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionResizedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionResizedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:session.resized:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionResizedV1Data {
    pub session_id: CommonIdV1,
    pub cols: EventSessionResizedV1DataCols,
    pub rows: EventSessionResizedV1DataRows,
}

/// `data.cols` in `urn:ads:event:session.resized:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionResizedV1DataCols(u16);

impl EventSessionResizedV1DataCols {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionResizedV1DataCols",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionResizedV1DataCols {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionResizedV1DataCols> for u16 {
    fn from(value: EventSessionResizedV1DataCols) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionResizedV1DataCols {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.rows` in `urn:ads:event:session.resized:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionResizedV1DataRows(u16);

impl EventSessionResizedV1DataRows {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionResizedV1DataRows",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionResizedV1DataRows {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionResizedV1DataRows> for u16 {
    fn from(value: EventSessionResizedV1DataRows) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionResizedV1DataRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
