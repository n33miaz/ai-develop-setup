// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.task_created.v1.schema.json. Não edite à mão.

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

/// Tarefa incluída na fila. Registra a inclusão de uma tarefa em queued, com zero tentativas.
///
/// `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventQueueTaskCreatedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueueTaskCreatedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueueTaskCreatedV1Data,
}

/// `type` in `urn:ads:event:queue.task_created:1`.
///
/// Always `"urn:ads:event:queue.task_created:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV1Type;

impl EventQueueTaskCreatedV1Type {
    pub const VALUE: &str = "urn:ads:event:queue.task_created:1";
}

impl Serialize for EventQueueTaskCreatedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_TASK_CREATED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_TASK_CREATED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueTaskCreatedV1Stream(String);

impl EventQueueTaskCreatedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueueTaskCreatedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_TASK_CREATED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_TASK_CREATED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueTaskCreatedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueTaskCreatedV1Stream> for String {
    fn from(value: EventQueueTaskCreatedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueTaskCreatedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueTaskCreatedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV1Data {
    pub queue_id: CommonIdV1,
    pub task_id: CommonIdV1,
    pub attempt_limit: EventQueueTaskCreatedV1DataAttemptLimit,
    pub origin: EventQueueTaskCreatedV1DataOrigin,
    pub workspace_alias: EventQueueTaskCreatedV1DataWorkspaceAlias,
    pub state: EventQueueTaskCreatedV1DataState,
    pub attempts: EventQueueTaskCreatedV1DataAttempts,
}

/// `data.attempt_limit` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventQueueTaskCreatedV1DataAttemptLimit(u16);

impl EventQueueTaskCreatedV1DataAttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV1DataAttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventQueueTaskCreatedV1DataAttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueueTaskCreatedV1DataAttemptLimit> for u16 {
    fn from(value: EventQueueTaskCreatedV1DataAttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV1DataAttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.origin` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EventQueueTaskCreatedV1DataOrigin {
    #[serde(rename = "manual")]
    Manual(EventQueueTaskCreatedV1DataOriginManual),
    #[serde(rename = "issue")]
    Issue(EventQueueTaskCreatedV1DataOriginIssue),
    #[serde(rename = "bug_rule")]
    BugRule(EventQueueTaskCreatedV1DataOriginBugRule),
}

/// `data.origin|manual` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV1DataOriginManual {
    pub request_id: CommonIdV1,
}

/// `data.origin|issue` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV1DataOriginIssue {
    pub r#ref: EventQueueTaskCreatedV1DataOriginIssueRef,
}

/// `data.origin|issue.ref` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct EventQueueTaskCreatedV1DataOriginIssueRef(u64);

impl EventQueueTaskCreatedV1DataOriginIssueRef {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV1DataOriginIssueRef",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for EventQueueTaskCreatedV1DataOriginIssueRef {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueueTaskCreatedV1DataOriginIssueRef> for u64 {
    fn from(value: EventQueueTaskCreatedV1DataOriginIssueRef) -> u64 {
        value.0
    }
}

impl Serialize for EventQueueTaskCreatedV1DataOriginIssueRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.origin|bug_rule` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueueTaskCreatedV1DataOriginBugRule {
    pub rule_id: CommonIdV1,
    pub target_id: CommonIdV1,
}

static EVENT_QUEUE_TASK_CREATED_V1_DATA_WORKSPACE_ALIAS_PATTERN: Pattern =
    Pattern::new(r"^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9_-])?$");
static EVENT_QUEUE_TASK_CREATED_V1_DATA_WORKSPACE_ALIAS_EXCLUDED: Pattern =
    Pattern::new(r"(^((con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|$)))|\.\.");

/// `data.workspace_alias` in `urn:ads:event:queue.task_created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueueTaskCreatedV1DataWorkspaceAlias(String);

impl EventQueueTaskCreatedV1DataWorkspaceAlias {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventQueueTaskCreatedV1DataWorkspaceAlias",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_TASK_CREATED_V1_DATA_WORKSPACE_ALIAS_PATTERN.matches(text)
            && !EVENT_QUEUE_TASK_CREATED_V1_DATA_WORKSPACE_ALIAS_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueueTaskCreatedV1DataWorkspaceAlias {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueueTaskCreatedV1DataWorkspaceAlias> for String {
    fn from(value: EventQueueTaskCreatedV1DataWorkspaceAlias) -> String {
        value.0
    }
}

impl fmt::Display for EventQueueTaskCreatedV1DataWorkspaceAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueueTaskCreatedV1DataWorkspaceAlias {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.state` in `urn:ads:event:queue.task_created:1`.
///
/// Always `"queued"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV1DataState;

impl EventQueueTaskCreatedV1DataState {
    pub const VALUE: &str = "queued";
}

impl Serialize for EventQueueTaskCreatedV1DataState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV1DataState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data.attempts` in `urn:ads:event:queue.task_created:1`.
///
/// Always `0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueueTaskCreatedV1DataAttempts;

impl EventQueueTaskCreatedV1DataAttempts {
    pub const VALUE: i64 = 0;
}

impl Serialize for EventQueueTaskCreatedV1DataAttempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueueTaskCreatedV1DataAttempts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
