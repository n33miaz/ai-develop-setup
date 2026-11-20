// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.queue-task.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_queue_task_state_v1::CommonQueueTaskStateV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Tarefa da fila. Retrato versionado de uma tarefa. A pasta é referida por apelido; o teto de tentativas inclui a primeira execução.
///
/// `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1 {
    pub version: RecordQueueTaskV1Version,
    pub id: CommonIdV1,
    pub queue_id: CommonIdV1,
    pub origin: RecordQueueTaskV1Origin,
    pub title: RecordQueueTaskV1Title,
    pub workspace_alias: RecordQueueTaskV1WorkspaceAlias,
    pub tool: CommonIdV1,
    pub state: CommonQueueTaskStateV1,
    pub attempts: RecordQueueTaskV1Attempts,
    pub attempt_limit: RecordQueueTaskV1AttemptLimit,
    pub created_at: CommonTimestampV1,
    pub updated_at: CommonTimestampV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub branch: Option<RecordQueueTaskV1Branch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub pull_request: Option<RecordQueueTaskV1PullRequest>,
}

/// `version` in `urn:ads:record:queue-task:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordQueueTaskV1Version;

impl RecordQueueTaskV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordQueueTaskV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordQueueTaskV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `origin` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RecordQueueTaskV1Origin {
    #[serde(rename = "manual")]
    Manual(RecordQueueTaskV1OriginManual),
    #[serde(rename = "issue")]
    Issue(RecordQueueTaskV1OriginIssue),
    #[serde(rename = "bug_rule")]
    BugRule(RecordQueueTaskV1OriginBugRule),
}

/// `origin|manual` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1OriginManual {
    pub request_id: CommonIdV1,
}

/// `origin|issue` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1OriginIssue {
    pub r#ref: RecordQueueTaskV1OriginIssueRef,
}

/// `origin|issue.ref` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV1OriginIssueRef(u64);

impl RecordQueueTaskV1OriginIssueRef {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1OriginIssueRef",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV1OriginIssueRef {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1OriginIssueRef> for u64 {
    fn from(value: RecordQueueTaskV1OriginIssueRef) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1OriginIssueRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `origin|bug_rule` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1OriginBugRule {
    pub rule_id: CommonIdV1,
    pub target_id: CommonIdV1,
}

static RECORD_QUEUE_TASK_V1_TITLE_PATTERN: Pattern = Pattern::new(
    r"^[^\x{0000}-\x{001f}\x{007f}-\x{009f}\x{061c}\x{200b}-\x{200f}\x{2028}-\x{202e}\x{2060}-\x{206f}\x{feff}]*$",
);

/// `title` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV1Title(String);

impl RecordQueueTaskV1Title {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV1Title", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=120).contains(&length) && RECORD_QUEUE_TASK_V1_TITLE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV1Title {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV1Title> for String {
    fn from(value: RecordQueueTaskV1Title) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV1Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV1Title {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_PATTERN: Pattern =
    Pattern::new(r"^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9_-])?$");
static RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_EXCLUDED: Pattern =
    Pattern::new(r"(^((con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|$)))|\.\.");

/// `workspace_alias` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV1WorkspaceAlias(String);

impl RecordQueueTaskV1WorkspaceAlias {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV1WorkspaceAlias", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_PATTERN.matches(text)
            && !RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV1WorkspaceAlias {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV1WorkspaceAlias> for String {
    fn from(value: RecordQueueTaskV1WorkspaceAlias) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV1WorkspaceAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV1WorkspaceAlias {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `attempts` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV1Attempts(u64);

impl RecordQueueTaskV1Attempts {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1Attempts",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV1Attempts {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1Attempts> for u64 {
    fn from(value: RecordQueueTaskV1Attempts) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1Attempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `attempt_limit` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct RecordQueueTaskV1AttemptLimit(u16);

impl RecordQueueTaskV1AttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1AttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for RecordQueueTaskV1AttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1AttemptLimit> for u16 {
    fn from(value: RecordQueueTaskV1AttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1AttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_QUEUE_TASK_V1_BRANCH_PATTERN: Pattern =
    Pattern::new(r"^(feature|fix)/[A-Za-z0-9][A-Za-z0-9._-]*(/[A-Za-z0-9][A-Za-z0-9._-]*)*$");
static RECORD_QUEUE_TASK_V1_BRANCH_EXCLUDED: Pattern = Pattern::new(r"\.\.|\.lock(/|$)|\.$");

/// `branch` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV1Branch(String);

impl RecordQueueTaskV1Branch {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV1Branch", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=255).contains(&length)
            && RECORD_QUEUE_TASK_V1_BRANCH_PATTERN.matches(text)
            && !RECORD_QUEUE_TASK_V1_BRANCH_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV1Branch {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV1Branch> for String {
    fn from(value: RecordQueueTaskV1Branch) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV1Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV1Branch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `pull_request` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV1PullRequest(u64);

impl RecordQueueTaskV1PullRequest {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1PullRequest",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV1PullRequest {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1PullRequest> for u64 {
    fn from(value: RecordQueueTaskV1PullRequest) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1PullRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
