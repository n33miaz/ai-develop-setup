// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.task_created.v2.schema.json. Não edite à mão.

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

static EVENT_QUEUE_TASK_CREATED_V2_RULES: &[(&str, Rule)] = &[(
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

/// Tarefa incluída na fila. Registra a inclusão de uma tarefa em queued, com zero tentativas. Uma criação de origem manual exige um aparelho como autor.
///
/// `urn:ads:event:queue.task_created:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueueTaskCreatedV2Fields")]
pub struct EventQueueTaskCreatedV2(EventQueueTaskCreatedV2Fields);

/// The fields of [`EventQueueTaskCreatedV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV2Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueueTaskCreatedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueTaskCreatedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueTaskCreatedV2Data,
}

impl EventQueueTaskCreatedV2 {
    pub fn new(fields: EventQueueTaskCreatedV2Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventQueueTaskCreatedV2",
            &fields,
            EVENT_QUEUE_TASK_CREATED_V2_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueueTaskCreatedV2Fields {
        self.0
    }
}

impl Deref for EventQueueTaskCreatedV2 {
    type Target = EventQueueTaskCreatedV2Fields;

    fn deref(&self) -> &EventQueueTaskCreatedV2Fields {
        &self.0
    }
}

impl TryFrom<EventQueueTaskCreatedV2Fields> for EventQueueTaskCreatedV2 {
    type Error = ContractError;

    fn try_from(fields: EventQueueTaskCreatedV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueueTaskCreatedV2> for EventQueueTaskCreatedV2Fields {
    fn from(value: EventQueueTaskCreatedV2) -> EventQueueTaskCreatedV2Fields {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.task_created:2`.
///
/// Always `"urn:ads:event:queue.task_created:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV2Type;

impl EventQueueTaskCreatedV2Type {
    pub const VALUE: &str = "urn:ads:event:queue.task_created:2";
}

impl Serialize for EventQueueTaskCreatedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_TASK_CREATED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_TASK_CREATED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueTaskCreatedV2Stream(String);

impl EventQueueTaskCreatedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueueTaskCreatedV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_TASK_CREATED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_TASK_CREATED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueTaskCreatedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueTaskCreatedV2Stream> for String {
    fn from(value: EventQueueTaskCreatedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueTaskCreatedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueTaskCreatedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV2Data {
    pub queue_id: CommonIdV1,
    pub task_id: CommonIdV1,
    pub attempt_limit: EventQueueTaskCreatedV2DataAttemptLimit,
    pub origin: EventQueueTaskCreatedV2DataOrigin,
    pub workspace_alias: EventQueueTaskCreatedV2DataWorkspaceAlias,
    pub state: EventQueueTaskCreatedV2DataState,
    pub attempts: EventQueueTaskCreatedV2DataAttempts,
}

/// `data.attempt_limit` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventQueueTaskCreatedV2DataAttemptLimit(u16);

impl EventQueueTaskCreatedV2DataAttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV2DataAttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventQueueTaskCreatedV2DataAttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueueTaskCreatedV2DataAttemptLimit> for u16 {
    fn from(value: EventQueueTaskCreatedV2DataAttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV2DataAttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.origin` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EventQueueTaskCreatedV2DataOrigin {
    #[serde(rename = "manual")]
    Manual(EventQueueTaskCreatedV2DataOriginManual),
    #[serde(rename = "issue")]
    Issue(EventQueueTaskCreatedV2DataOriginIssue),
    #[serde(rename = "bug_rule")]
    BugRule(EventQueueTaskCreatedV2DataOriginBugRule),
}

/// `data.origin|manual` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV2DataOriginManual {
    pub request_id: CommonIdV1,
}

/// `data.origin|issue` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV2DataOriginIssue {
    pub r#ref: EventQueueTaskCreatedV2DataOriginIssueRef,
}

/// `data.origin|issue.ref` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct EventQueueTaskCreatedV2DataOriginIssueRef(u64);

impl EventQueueTaskCreatedV2DataOriginIssueRef {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV2DataOriginIssueRef",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for EventQueueTaskCreatedV2DataOriginIssueRef {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueueTaskCreatedV2DataOriginIssueRef> for u64 {
    fn from(value: EventQueueTaskCreatedV2DataOriginIssueRef) -> u64 {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV2DataOriginIssueRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.origin|bug_rule` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV2DataOriginBugRule {
    pub rule_id: CommonIdV1,
    pub target_id: CommonIdV1,
}

static EVENT_QUEUE_TASK_CREATED_V2_DATA_WORKSPACE_ALIAS_PATTERN: Pattern =
    Pattern::new(r"^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9_-])?$");
static EVENT_QUEUE_TASK_CREATED_V2_DATA_WORKSPACE_ALIAS_EXCLUDED: Pattern =
    Pattern::new(r"(^((con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|$)))|\.\.");

/// `data.workspace_alias` in `urn:ads:event:queue.task_created:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueTaskCreatedV2DataWorkspaceAlias(String);

impl EventQueueTaskCreatedV2DataWorkspaceAlias {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV2DataWorkspaceAlias",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_TASK_CREATED_V2_DATA_WORKSPACE_ALIAS_PATTERN.matches(text)
            && !EVENT_QUEUE_TASK_CREATED_V2_DATA_WORKSPACE_ALIAS_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueTaskCreatedV2DataWorkspaceAlias {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueTaskCreatedV2DataWorkspaceAlias> for String {
    fn from(value: EventQueueTaskCreatedV2DataWorkspaceAlias) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueTaskCreatedV2DataWorkspaceAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueTaskCreatedV2DataWorkspaceAlias {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.state` in `urn:ads:event:queue.task_created:2`.
///
/// Always `"queued"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV2DataState;

impl EventQueueTaskCreatedV2DataState {
    pub const VALUE: &str = "queued";
}

impl Serialize for EventQueueTaskCreatedV2DataState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV2DataState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data.attempts` in `urn:ads:event:queue.task_created:2`.
///
/// Always `0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV2DataAttempts;

impl EventQueueTaskCreatedV2DataAttempts {
    pub const VALUE: i64 = 0;
}

impl Serialize for EventQueueTaskCreatedV2DataAttempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV2DataAttempts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
