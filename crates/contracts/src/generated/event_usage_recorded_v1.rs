// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.usage.recorded.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use super::record_usage_event_v1::RecordUsageEventV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Uso registrado. Uma chamada ou turno de IA foi medido: o dado é um registro usage_event completo, gravado só-anexo no banco de métricas.
///
/// `urn:ads:event:usage.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventUsageRecordedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventUsageRecordedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventUsageRecordedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: RecordUsageEventV1,
}

/// `type` in `urn:ads:event:usage.recorded:1`.
///
/// Always `"urn:ads:event:usage.recorded:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventUsageRecordedV1Type;

impl EventUsageRecordedV1Type {
    pub const VALUE: &str = "urn:ads:event:usage.recorded:1";
}

impl Serialize for EventUsageRecordedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventUsageRecordedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_USAGE_RECORDED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_USAGE_RECORDED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^usage/");

/// `stream` in `urn:ads:event:usage.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventUsageRecordedV1Stream(String);

impl EventUsageRecordedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventUsageRecordedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_USAGE_RECORDED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_USAGE_RECORDED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventUsageRecordedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventUsageRecordedV1Stream> for String {
    fn from(value: EventUsageRecordedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventUsageRecordedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventUsageRecordedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
