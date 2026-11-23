// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.task_created.v3.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_queue_task_origin_v1::CommonQueueTaskOriginV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use super::common_workdir_alias_v1::CommonWorkdirAliasV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_QUEUE_TASK_CREATED_V3_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/if",
    Rule::If {
        when: &Rule::Properties(&[(
            "data",
            Rule::AllOf(&[
                Rule::Type(&[JsonType::Object]),
                Rule::Properties(&[(
                    "origin",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Properties(&[("kind", Rule::Const(Literal::Str("manual")))]),
                    ]),
                )]),
            ]),
        )]),
        then: Some(&Rule::Properties(&[(
            "actor",
            Rule::AllOf(&[
                Rule::Type(&[JsonType::Object]),
                Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
            ]),
        )])),
        otherwise: None,
    },
)];

/// Tarefa incluída na fila. Registra a inclusão de uma tarefa em queued, com zero tentativas. Uma criação de origem manual exige um aparelho como autor. O apelido e a origem são os tipos comuns.
///
/// `urn:ads:event:queue.task_created:3`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueTaskCreatedV3Fields")]
pub struct EventQueueTaskCreatedV3(EventQueueTaskCreatedV3Fields);

/// The fields of [`EventQueueTaskCreatedV3`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV3Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueueTaskCreatedV3Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueTaskCreatedV3Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueTaskCreatedV3Data,
}

impl EventQueueTaskCreatedV3 {
    pub fn new(fields: EventQueueTaskCreatedV3Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventQueueTaskCreatedV3",
            &fields,
            EVENT_QUEUE_TASK_CREATED_V3_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueTaskCreatedV3Fields {
        self.0
    }
}

impl Deref for EventQueueTaskCreatedV3 {
    type Target = EventQueueTaskCreatedV3Fields;

    fn deref(&self) -> &EventQueueTaskCreatedV3Fields {
        &self.0
    }
}

impl TryFrom<EventQueueTaskCreatedV3Fields> for EventQueueTaskCreatedV3 {
    type Error = ContractError;

    fn try_from(fields: EventQueueTaskCreatedV3Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueTaskCreatedV3> for EventQueueTaskCreatedV3Fields {
    fn from(value: EventQueueTaskCreatedV3) -> EventQueueTaskCreatedV3Fields {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV3 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.task_created:3`.
///
/// Always `"urn:ads:event:queue.task_created:3"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV3Type;

impl EventQueueTaskCreatedV3Type {
    pub const VALUE: &str = "urn:ads:event:queue.task_created:3";
}

impl Serialize for EventQueueTaskCreatedV3Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV3Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_TASK_CREATED_V3_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_TASK_CREATED_V3_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.task_created:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueTaskCreatedV3Stream(String);

impl EventQueueTaskCreatedV3Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueueTaskCreatedV3Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_TASK_CREATED_V3_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_TASK_CREATED_V3_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueTaskCreatedV3Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueTaskCreatedV3Stream> for String {
    fn from(value: EventQueueTaskCreatedV3Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueTaskCreatedV3Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueTaskCreatedV3Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.task_created:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV3Data {
    pub queue_id: CommonIdV1,
    pub task_id: CommonIdV1,
    pub attempt_limit: EventQueueTaskCreatedV3DataAttemptLimit,
    pub origin: CommonQueueTaskOriginV1,
    pub workspace_alias: CommonWorkdirAliasV1,
    pub state: EventQueueTaskCreatedV3DataState,
    pub attempts: EventQueueTaskCreatedV3DataAttempts,
}

/// `data.attempt_limit` in `urn:ads:event:queue.task_created:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventQueueTaskCreatedV3DataAttemptLimit(u16);

impl EventQueueTaskCreatedV3DataAttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV3DataAttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventQueueTaskCreatedV3DataAttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueueTaskCreatedV3DataAttemptLimit> for u16 {
    fn from(value: EventQueueTaskCreatedV3DataAttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV3DataAttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.state` in `urn:ads:event:queue.task_created:3`.
///
/// Always `"queued"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV3DataState;

impl EventQueueTaskCreatedV3DataState {
    pub const VALUE: &str = "queued";
}

impl Serialize for EventQueueTaskCreatedV3DataState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV3DataState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data.attempts` in `urn:ads:event:queue.task_created:3`.
///
/// Always `0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV3DataAttempts;

impl EventQueueTaskCreatedV3DataAttempts {
    pub const VALUE: i64 = 0;
}

impl Serialize for EventQueueTaskCreatedV3DataAttempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV3DataAttempts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
