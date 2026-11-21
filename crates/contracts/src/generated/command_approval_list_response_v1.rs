// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.list.response.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use super::record_action_plan_v2::RecordActionPlanV2;
use crate::support;
use crate::support::ContractError;

/// Resposta de approval.list. Resposta JSON-RPC 2.0 aceita de approval.list: as ações que aguardam a resposta do dono, da mais antiga à mais nova, cada uma com a aprovação pedida e o registro do plano como o portão o julgou (alvos, destino, quarentena, estimativa e regras). A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:approval.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV1 {
    pub jsonrpc: CommandApprovalListResponseV1Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandApprovalListResponseV1Result,
}

/// `jsonrpc` in `urn:ads:command:approval.list.response:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalListResponseV1Jsonrpc;

impl CommandApprovalListResponseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalListResponseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalListResponseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:approval.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV1Result {
    pub approvals: CommandApprovalListResponseV1ResultApprovals,
}

/// `result.approvals` in `urn:ads:command:approval.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV1ResultApprovalsItem>")]
pub struct CommandApprovalListResponseV1ResultApprovals(
    Vec<CommandApprovalListResponseV1ResultApprovalsItem>,
);

impl CommandApprovalListResponseV1ResultApprovals {
    pub const MAX_ITEMS: usize = 1000;

    pub fn new(
        items: Vec<CommandApprovalListResponseV1ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV1ResultApprovals",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV1ResultApprovalsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommandApprovalListResponseV1ResultApprovalsItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV1ResultApprovalsItem>>
    for CommandApprovalListResponseV1ResultApprovals
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV1ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV1ResultApprovals>
    for Vec<CommandApprovalListResponseV1ResultApprovalsItem>
{
    fn from(
        value: CommandApprovalListResponseV1ResultApprovals,
    ) -> Vec<CommandApprovalListResponseV1ResultApprovalsItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV1ResultApprovals {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[]` in `urn:ads:command:approval.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV1ResultApprovalsItem {
    pub action_id: CommonIdV1,
    pub approval_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub risk: CommonRiskV1,
    pub summary: CommandApprovalListResponseV1ResultApprovalsItemSummary,
    pub requested_at: CommonTimestampV1,
    pub expires_at: CommonTimestampV1,
    pub plan: RecordActionPlanV2,
}

/// `result.approvals[].summary` in `urn:ads:command:approval.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV1ResultApprovalsItemSummary(String);

impl CommandApprovalListResponseV1ResultApprovalsItemSummary {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV1ResultApprovalsItemSummary",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalListResponseV1ResultApprovalsItemSummary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV1ResultApprovalsItemSummary> for String {
    fn from(value: CommandApprovalListResponseV1ResultApprovalsItemSummary) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV1ResultApprovalsItemSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV1ResultApprovalsItemSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
