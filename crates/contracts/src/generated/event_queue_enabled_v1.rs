// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.enabled.v1.schema.json. Não edite à mão.

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

static EVENT_QUEUE_ENABLED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Fila ligada pelo dono. Registra a ativação local solicitada pelo dono autenticado.
///
/// `urn:ads:event:queue.enabled:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueEnabledV1Fields")]
pub struct EventQueueEnabledV1(EventQueueEnabledV1Fields);

/// The fields of [`EventQueueEnabledV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueEnabledV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueueEnabledV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueEnabledV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueEnabledV1Data,
}

impl EventQueueEnabledV1 {
    pub fn new(fields: EventQueueEnabledV1Fields) -> Result<Self, ContractError> {
        support::check_rules("EventQueueEnabledV1", &fields, EVENT_QUEUE_ENABLED_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueEnabledV1Fields {
        self.0
    }
}

impl Deref for EventQueueEnabledV1 {
    type Target = EventQueueEnabledV1Fields;

    fn deref(&self) -> &EventQueueEnabledV1Fields {
        &self.0
    }
}

impl TryFrom<EventQueueEnabledV1Fields> for EventQueueEnabledV1 {
    type Error = ContractError;

    fn try_from(fields: EventQueueEnabledV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueEnabledV1> for EventQueueEnabledV1Fields {
    fn from(value: EventQueueEnabledV1) -> EventQueueEnabledV1Fields {
        value.0
    }
}

impl Serialize for EventQueueEnabledV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.enabled:1`.
///
/// Always `"urn:ads:event:queue.enabled:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueEnabledV1Type;

impl EventQueueEnabledV1Type {
    pub const VALUE: &str = "urn:ads:event:queue.enabled:1";
}

impl Serialize for EventQueueEnabledV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueEnabledV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_ENABLED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_ENABLED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.enabled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueEnabledV1Stream(String);

impl EventQueueEnabledV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueueEnabledV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_ENABLED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_ENABLED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueEnabledV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueEnabledV1Stream> for String {
    fn from(value: EventQueueEnabledV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueEnabledV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueEnabledV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.enabled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueEnabledV1Data {
    pub queue_id: CommonIdV1,
    pub owner_id: CommonIdV1,
    pub via: EventQueueEnabledV1DataVia,
}

/// `data.via` in `urn:ads:event:queue.enabled:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventQueueEnabledV1DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
}

impl EventQueueEnabledV1DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
        }
    }
}
