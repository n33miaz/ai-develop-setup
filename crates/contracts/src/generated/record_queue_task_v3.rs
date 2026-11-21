// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.queue-task.v3.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_queue_task_origin_v1::CommonQueueTaskOriginV1;
use super::common_queue_task_state_v1::CommonQueueTaskStateV1;
use super::common_queue_task_title_v1::CommonQueueTaskTitleV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_workdir_alias_v1::CommonWorkdirAliasV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Tarefa da fila. Retrato versionado de uma tarefa, como o replay o reconstrói do log. A pasta é referida por apelido; o teto de tentativas inclui a primeira execução. title e tool são nulos quando a tarefa foi criada por queue.task_created anterior à :4, que não os registrava: o registro não inventa valor.
///
/// `urn:ads:record:queue-task:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV3 {
    pub version: RecordQueueTaskV3Version,
    pub id: CommonIdV1,
    pub queue_id: CommonIdV1,
    pub origin: CommonQueueTaskOriginV1,
    #[serde(deserialize_with = "support::nullable")]
    pub title: Option<CommonQueueTaskTitleV1>,
    pub workspace_alias: CommonWorkdirAliasV1,
    #[serde(deserialize_with = "support::nullable")]
    pub tool: Option<CommonIdV1>,
    pub state: CommonQueueTaskStateV1,
    pub attempts: RecordQueueTaskV3Attempts,
    pub attempt_limit: RecordQueueTaskV3AttemptLimit,
    pub created_at: CommonTimestampV1,
    pub updated_at: CommonTimestampV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub branch: Option<RecordQueueTaskV3Branch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub pull_request: Option<RecordQueueTaskV3PullRequest>,
}

/// `version` in `urn:ads:record:queue-task:3`.
///
/// Always `3`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordQueueTaskV3Version;

impl RecordQueueTaskV3Version {
    pub const VALUE: i64 = 3;
}

impl Serialize for RecordQueueTaskV3Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordQueueTaskV3Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `attempts` in `urn:ads:record:queue-task:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV3Attempts(u64);

impl RecordQueueTaskV3Attempts {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV3Attempts",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV3Attempts {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV3Attempts> for u64 {
    fn from(value: RecordQueueTaskV3Attempts) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV3Attempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `attempt_limit` in `urn:ads:record:queue-task:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct RecordQueueTaskV3AttemptLimit(u16);

impl RecordQueueTaskV3AttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV3AttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for RecordQueueTaskV3AttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV3AttemptLimit> for u16 {
    fn from(value: RecordQueueTaskV3AttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV3AttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_QUEUE_TASK_V3_BRANCH_PATTERN: Pattern =
    Pattern::new(r"^(feature|fix)/[a-z0-9][a-z0-9._-]*(/[a-z0-9][a-z0-9._-]*)*$");
static RECORD_QUEUE_TASK_V3_BRANCH_EXCLUDED: Pattern =
    Pattern::new(r"\.\.|\.lock(/|$)|\.(/|$)|(^|/)(con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|/|$)");

/// `branch` in `urn:ads:record:queue-task:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV3Branch(String);

impl RecordQueueTaskV3Branch {
    pub const MAX_CHARS: usize = 255;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV3Branch", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
            && RECORD_QUEUE_TASK_V3_BRANCH_PATTERN.matches(text)
            && !RECORD_QUEUE_TASK_V3_BRANCH_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV3Branch {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV3Branch> for String {
    fn from(value: RecordQueueTaskV3Branch) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV3Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV3Branch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `pull_request` in `urn:ads:record:queue-task:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV3PullRequest(u64);

impl RecordQueueTaskV3PullRequest {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV3PullRequest",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV3PullRequest {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV3PullRequest> for u64 {
    fn from(value: RecordQueueTaskV3PullRequest) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV3PullRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
