// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.queue-task-origin.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use crate::support::ContractError;

/// Origem de uma tarefa da fila. De onde veio a tarefa: pedido manual (id estável do pedido), issue (número no repositório do apelido) ou regra de bug (par regra e alvo estável).
///
/// `urn:ads:common:queue-task-origin:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CommonQueueTaskOriginV1 {
    #[serde(rename = "manual")]
    Manual(CommonQueueTaskOriginV1Manual),
    #[serde(rename = "issue")]
    Issue(CommonQueueTaskOriginV1Issue),
    #[serde(rename = "bug_rule")]
    BugRule(CommonQueueTaskOriginV1BugRule),
}

/// `manual` in `urn:ads:common:queue-task-origin:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonQueueTaskOriginV1Manual {
    pub request_id: CommonIdV1,
}

/// `issue` in `urn:ads:common:queue-task-origin:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonQueueTaskOriginV1Issue {
    pub r#ref: CommonQueueTaskOriginV1IssueRef,
}

/// `issue.ref` in `urn:ads:common:queue-task-origin:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct CommonQueueTaskOriginV1IssueRef(u64);

impl CommonQueueTaskOriginV1IssueRef {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "CommonQueueTaskOriginV1IssueRef",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for CommonQueueTaskOriginV1IssueRef {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<CommonQueueTaskOriginV1IssueRef> for u64 {
    fn from(value: CommonQueueTaskOriginV1IssueRef) -> u64 {
        value.0
    }
}

impl Serialize for CommonQueueTaskOriginV1IssueRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `bug_rule` in `urn:ads:common:queue-task-origin:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonQueueTaskOriginV1BugRule {
    pub rule_id: CommonIdV1,
    pub target_id: CommonIdV1,
}
