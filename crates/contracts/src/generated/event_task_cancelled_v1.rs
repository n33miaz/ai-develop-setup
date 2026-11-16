// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.task.cancelled.v1.schema.json. Não edite à mão.

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

/// Tarefa cancelada. A tarefa foi interrompida por decisão do dono, por teto de custo, por pressão de recursos, por política ou por ter sido substituída.
///
/// `urn:ads:event:task.cancelled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTaskCancelledV1 {
    pub id: CommonUlidV1,
    pub r#type: EventTaskCancelledV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventTaskCancelledV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventTaskCancelledV1Data,
}

/// `type` in `urn:ads:event:task.cancelled:1`.
///
/// Always `"urn:ads:event:task.cancelled:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventTaskCancelledV1Type;

impl EventTaskCancelledV1Type {
    pub const VALUE: &str = "urn:ads:event:task.cancelled:1";
}

impl Serialize for EventTaskCancelledV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventTaskCancelledV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_TASK_CANCELLED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_TASK_CANCELLED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^task/");

/// `stream` in `urn:ads:event:task.cancelled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventTaskCancelledV1Stream(String);

impl EventTaskCancelledV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventTaskCancelledV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_TASK_CANCELLED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_TASK_CANCELLED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventTaskCancelledV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventTaskCancelledV1Stream> for String {
    fn from(value: EventTaskCancelledV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventTaskCancelledV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventTaskCancelledV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:task.cancelled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTaskCancelledV1Data {
    pub task_id: CommonIdV1,
    pub reason: EventTaskCancelledV1DataReason,
}

/// `data.reason` in `urn:ads:event:task.cancelled:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventTaskCancelledV1DataReason {
    #[serde(rename = "owner")]
    Owner,
    #[serde(rename = "budget")]
    Budget,
    #[serde(rename = "host_pressure")]
    HostPressure,
    #[serde(rename = "policy")]
    Policy,
    #[serde(rename = "superseded")]
    Superseded,
}

impl EventTaskCancelledV1DataReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Budget => "budget",
            Self::HostPressure => "host_pressure",
            Self::Policy => "policy",
            Self::Superseded => "superseded",
        }
    }
}
