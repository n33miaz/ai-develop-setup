// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.stopped.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_QUEUE_STOPPED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Fila parada pelo dono. Registra a parada solicitada pelo dono autenticado, identificado em data.owner_id.
///
/// `urn:ads:event:queue.stopped:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueStoppedV1Fields")]
pub struct EventQueueStoppedV1(EventQueueStoppedV1Fields);

/// The fields of [`EventQueueStoppedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueStoppedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueueStoppedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueStoppedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueStoppedV1Data,
}

impl EventQueueStoppedV1 {
    pub fn new(fields: EventQueueStoppedV1Fields) -> Result<Self, ContractError> {
        support::check_rules("EventQueueStoppedV1", &fields, EVENT_QUEUE_STOPPED_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueStoppedV1Fields {
        self.0
    }
}

impl Deref for EventQueueStoppedV1 {
    type Target = EventQueueStoppedV1Fields;

    fn deref(&self) -> &EventQueueStoppedV1Fields {
        &self.0
    }
}

impl TryFrom<EventQueueStoppedV1Fields> for EventQueueStoppedV1 {
    type Error = ContractError;

    fn try_from(fields: EventQueueStoppedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueStoppedV1> for EventQueueStoppedV1Fields {
    fn from(value: EventQueueStoppedV1) -> EventQueueStoppedV1Fields {
        value.0
    }
}

impl Serialize for EventQueueStoppedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.stopped:1`.
///
/// Always `"urn:ads:event:queue.stopped:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueStoppedV1Type;

impl EventQueueStoppedV1Type {
    pub const VALUE: &str = "urn:ads:event:queue.stopped:1";
}

impl Serialize for EventQueueStoppedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueStoppedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_STOPPED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_STOPPED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.stopped:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueStoppedV1Stream(String);

impl EventQueueStoppedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueueStoppedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_STOPPED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_STOPPED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueStoppedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueStoppedV1Stream> for String {
    fn from(value: EventQueueStoppedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueStoppedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueStoppedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.stopped:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueStoppedV1Data {
    pub queue_id: CommonIdV1,
    pub owner_id: CommonIdV1,
    pub via: EventQueueStoppedV1DataVia,
}

/// `data.via` in `urn:ads:event:queue.stopped:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventQueueStoppedV1DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl EventQueueStoppedV1DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}
