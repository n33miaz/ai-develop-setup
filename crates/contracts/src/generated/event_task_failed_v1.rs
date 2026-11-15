// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.task.failed.v1.schema.json. Não edite à mão.

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

/// Tarefa falhou. A tarefa terminou sem entrega, com um código de erro estável e a indicação de se vale tentar de novo.
///
/// `urn:ads:event:task.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTaskFailedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventTaskFailedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventTaskFailedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventTaskFailedV1Data,
}

/// `type` in `urn:ads:event:task.failed:1`.
///
/// Always `"urn:ads:event:task.failed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventTaskFailedV1Type;

impl EventTaskFailedV1Type {
    pub const VALUE: &str = "urn:ads:event:task.failed:1";
}

impl Serialize for EventTaskFailedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventTaskFailedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_TASK_FAILED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_TASK_FAILED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^task/");

/// `stream` in `urn:ads:event:task.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventTaskFailedV1Stream(String);

impl EventTaskFailedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventTaskFailedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_TASK_FAILED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_TASK_FAILED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventTaskFailedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventTaskFailedV1Stream> for String {
    fn from(value: EventTaskFailedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventTaskFailedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventTaskFailedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:task.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTaskFailedV1Data {
    pub task_id: CommonIdV1,
    pub error_code: EventTaskFailedV1DataErrorCode,
    pub retriable: bool,
}

static EVENT_TASK_FAILED_V1_DATA_ERROR_CODE_PATTERN: Pattern = Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `data.error_code` in `urn:ads:event:task.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventTaskFailedV1DataErrorCode(String);

impl EventTaskFailedV1DataErrorCode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventTaskFailedV1DataErrorCode", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_TASK_FAILED_V1_DATA_ERROR_CODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventTaskFailedV1DataErrorCode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventTaskFailedV1DataErrorCode> for String {
    fn from(value: EventTaskFailedV1DataErrorCode) -> String {
        value.0
    }
}

impl fmt::Display for EventTaskFailedV1DataErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventTaskFailedV1DataErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
