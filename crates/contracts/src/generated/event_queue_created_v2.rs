// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.created.v2.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_queue_policy_v1::CommonQueuePolicyV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_QUEUE_CREATED_V2_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Fila criada. Registra a fila inicial parada, a identidade cadastrada do dono e a política inicial.
///
/// `urn:ads:event:queue.created:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueCreatedV2Fields")]
pub struct EventQueueCreatedV2(EventQueueCreatedV2Fields);

/// The fields of [`EventQueueCreatedV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueCreatedV2Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueueCreatedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueCreatedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueCreatedV2Data,
}

impl EventQueueCreatedV2 {
    pub fn new(fields: EventQueueCreatedV2Fields) -> Result<Self, ContractError> {
        support::check_rules("EventQueueCreatedV2", &fields, EVENT_QUEUE_CREATED_V2_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueCreatedV2Fields {
        self.0
    }
}

impl Deref for EventQueueCreatedV2 {
    type Target = EventQueueCreatedV2Fields;

    fn deref(&self) -> &EventQueueCreatedV2Fields {
        &self.0
    }
}

impl TryFrom<EventQueueCreatedV2Fields> for EventQueueCreatedV2 {
    type Error = ContractError;

    fn try_from(fields: EventQueueCreatedV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueCreatedV2> for EventQueueCreatedV2Fields {
    fn from(value: EventQueueCreatedV2) -> EventQueueCreatedV2Fields {
        value.0
    }
}

impl Serialize for EventQueueCreatedV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.created:2`.
///
/// Always `"urn:ads:event:queue.created:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueCreatedV2Type;

impl EventQueueCreatedV2Type {
    pub const VALUE: &str = "urn:ads:event:queue.created:2";
}

impl Serialize for EventQueueCreatedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueCreatedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_CREATED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_CREATED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueCreatedV2Stream(String);

impl EventQueueCreatedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueueCreatedV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_CREATED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_CREATED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueCreatedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueCreatedV2Stream> for String {
    fn from(value: EventQueueCreatedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueCreatedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueCreatedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueCreatedV2Data {
    pub queue_id: CommonIdV1,
    pub owner_id: CommonIdV1,
    pub via: EventQueueCreatedV2DataVia,
    pub state: EventQueueCreatedV2DataState,
    pub policy: CommonQueuePolicyV1,
}

/// `data.via` in `urn:ads:event:queue.created:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventQueueCreatedV2DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl EventQueueCreatedV2DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}

/// `data.state` in `urn:ads:event:queue.created:2`.
///
/// Always `"stopped"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueCreatedV2DataState;

impl EventQueueCreatedV2DataState {
    pub const VALUE: &str = "stopped";
}

impl Serialize for EventQueueCreatedV2DataState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueCreatedV2DataState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}
