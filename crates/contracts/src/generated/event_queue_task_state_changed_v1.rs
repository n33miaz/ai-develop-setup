// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.task_state_changed.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_queue_task_state_v1::CommonQueueTaskStateV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_QUEUE_TASK_STATE_CHANGED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/if",
    Rule::If {
        when: &Rule::Properties(&[(
            "data",
            Rule::AllOf(&[
                Rule::Properties(&[
                    ("from", Rule::Const(Literal::Str("review"))),
                    ("to", Rule::Const(Literal::Str("done"))),
                ]),
                Rule::Type(&[JsonType::Object]),
            ]),
        )]),
        then: Some(&Rule::Properties(&[
            (
                "actor",
                Rule::AllOf(&[
                    Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
                    Rule::Type(&[JsonType::Object]),
                ]),
            ),
            (
                "data",
                Rule::AllOf(&[Rule::Required(&["via"]), Rule::Type(&[JsonType::Object])]),
            ),
        ])),
        otherwise: None,
    },
)];

/// Estado da tarefa da fila alterado. Registra os estados anterior e posterior e a contagem de tentativas após a mudança.
///
/// `urn:ads:event:queue.task_state_changed:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueTaskStateChangedV1Fields")]
pub struct EventQueueTaskStateChangedV1(EventQueueTaskStateChangedV1Fields);

/// The fields of [`EventQueueTaskStateChangedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskStateChangedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueueTaskStateChangedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueTaskStateChangedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueTaskStateChangedV1Data,
}

impl EventQueueTaskStateChangedV1 {
    pub fn new(fields: EventQueueTaskStateChangedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventQueueTaskStateChangedV1",
            &fields,
            EVENT_QUEUE_TASK_STATE_CHANGED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueTaskStateChangedV1Fields {
        self.0
    }
}

impl Deref for EventQueueTaskStateChangedV1 {
    type Target = EventQueueTaskStateChangedV1Fields;

    fn deref(&self) -> &EventQueueTaskStateChangedV1Fields {
        &self.0
    }
}

impl TryFrom<EventQueueTaskStateChangedV1Fields> for EventQueueTaskStateChangedV1 {
    type Error = ContractError;

    fn try_from(fields: EventQueueTaskStateChangedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueTaskStateChangedV1> for EventQueueTaskStateChangedV1Fields {
    fn from(value: EventQueueTaskStateChangedV1) -> EventQueueTaskStateChangedV1Fields {
        value.0
    }
}

impl Serialize for EventQueueTaskStateChangedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.task_state_changed:1`.
///
/// Always `"urn:ads:event:queue.task_state_changed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskStateChangedV1Type;

impl EventQueueTaskStateChangedV1Type {
    pub const VALUE: &str = "urn:ads:event:queue.task_state_changed:1";
}

impl Serialize for EventQueueTaskStateChangedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskStateChangedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_TASK_STATE_CHANGED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_TASK_STATE_CHANGED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.task_state_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueTaskStateChangedV1Stream(String);

impl EventQueueTaskStateChangedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventQueueTaskStateChangedV1Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_TASK_STATE_CHANGED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_TASK_STATE_CHANGED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueTaskStateChangedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueTaskStateChangedV1Stream> for String {
    fn from(value: EventQueueTaskStateChangedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueTaskStateChangedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueTaskStateChangedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static EVENT_QUEUE_TASK_STATE_CHANGED_V1_DATA_RULES: &[(&str, Rule)] = &[
    (
        "#/properties/data/allOf/0",
        Rule::If {
            when: &Rule::Properties(&[("from", Rule::Const(Literal::Str("queued")))]),
            then: Some(&Rule::Properties(&[(
                "to",
                Rule::Enum(&[Literal::Str("running"), Literal::Str("cancelled")]),
            )])),
            otherwise: None,
        },
    ),
    (
        "#/properties/data/allOf/1",
        Rule::If {
            when: &Rule::Properties(&[("from", Rule::Const(Literal::Str("running")))]),
            then: Some(&Rule::Properties(&[(
                "to",
                Rule::Enum(&[
                    Literal::Str("review"),
                    Literal::Str("failed"),
                    Literal::Str("cancelled"),
                ]),
            )])),
            otherwise: None,
        },
    ),
    (
        "#/properties/data/allOf/2",
        Rule::If {
            when: &Rule::Properties(&[("from", Rule::Const(Literal::Str("review")))]),
            then: Some(&Rule::Properties(&[(
                "to",
                Rule::Enum(&[
                    Literal::Str("done"),
                    Literal::Str("failed"),
                    Literal::Str("cancelled"),
                ]),
            )])),
            otherwise: None,
        },
    ),
    (
        "#/properties/data/allOf/3",
        Rule::If {
            when: &Rule::Properties(&[("from", Rule::Const(Literal::Str("failed")))]),
            then: Some(&Rule::Properties(&[(
                "to",
                Rule::Enum(&[Literal::Str("queued"), Literal::Str("cancelled")]),
            )])),
            otherwise: None,
        },
    ),
    (
        "#/properties/data/allOf/4",
        Rule::If {
            when: &Rule::Properties(&[("from", Rule::Const(Literal::Str("done")))]),
            then: Some(&Rule::Not(&Rule::Required(&["to"]))),
            otherwise: None,
        },
    ),
    (
        "#/properties/data/allOf/5",
        Rule::If {
            when: &Rule::Properties(&[("from", Rule::Const(Literal::Str("cancelled")))]),
            then: Some(&Rule::Not(&Rule::Required(&["to"]))),
            otherwise: None,
        },
    ),
    (
        "#/properties/data/allOf/6",
        Rule::If {
            when: &Rule::Properties(&[("to", Rule::Const(Literal::Str("running")))]),
            then: Some(&Rule::Properties(&[(
                "attempts",
                Rule::AllOf(&[Rule::Minimum(1.0), Rule::Type(&[JsonType::Integer])]),
            )])),
            otherwise: None,
        },
    ),
];

/// `data` in `urn:ads:event:queue.task_state_changed:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueTaskStateChangedV1DataFields")]
pub struct EventQueueTaskStateChangedV1Data(EventQueueTaskStateChangedV1DataFields);

/// The fields of [`EventQueueTaskStateChangedV1Data`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskStateChangedV1DataFields {
    pub queue_id: CommonIdV1,
    pub task_id: CommonIdV1,
    pub from: CommonQueueTaskStateV1,
    pub to: CommonQueueTaskStateV1,
    pub attempts: EventQueueTaskStateChangedV1DataAttempts,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub via: Option<EventQueueTaskStateChangedV1DataVia>,
}

impl EventQueueTaskStateChangedV1Data {
    pub fn new(fields: EventQueueTaskStateChangedV1DataFields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventQueueTaskStateChangedV1Data",
            &fields,
            EVENT_QUEUE_TASK_STATE_CHANGED_V1_DATA_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueTaskStateChangedV1DataFields {
        self.0
    }
}

impl Deref for EventQueueTaskStateChangedV1Data {
    type Target = EventQueueTaskStateChangedV1DataFields;

    fn deref(&self) -> &EventQueueTaskStateChangedV1DataFields {
        &self.0
    }
}

impl TryFrom<EventQueueTaskStateChangedV1DataFields> for EventQueueTaskStateChangedV1Data {
    type Error = ContractError;

    fn try_from(fields: EventQueueTaskStateChangedV1DataFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueTaskStateChangedV1Data> for EventQueueTaskStateChangedV1DataFields {
    fn from(value: EventQueueTaskStateChangedV1Data) -> EventQueueTaskStateChangedV1DataFields {
        value.0
    }
}

impl Serialize for EventQueueTaskStateChangedV1Data {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.attempts` in `urn:ads:event:queue.task_state_changed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct EventQueueTaskStateChangedV1DataAttempts(u64);

impl EventQueueTaskStateChangedV1DataAttempts {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueueTaskStateChangedV1DataAttempts",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for EventQueueTaskStateChangedV1DataAttempts {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueueTaskStateChangedV1DataAttempts> for u64 {
    fn from(value: EventQueueTaskStateChangedV1DataAttempts) -> u64 {
        value.0
    }
}

impl Serialize for EventQueueTaskStateChangedV1DataAttempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.via` in `urn:ads:event:queue.task_state_changed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventQueueTaskStateChangedV1DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl EventQueueTaskStateChangedV1DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}
