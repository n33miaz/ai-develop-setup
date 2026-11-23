// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.list.response.v2.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use super::record_action_effects_v1::RecordActionEffectsV1;
use super::record_action_plan_v2::RecordActionPlanV2;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Resposta de approval.list. Resposta JSON-RPC 2.0 aceita de approval.list: as ações que aguardam a resposta do dono, da mais antiga à mais nova, cada uma com a aprovação pedida, o registro do plano como o portão o julgou (alvos como pedidos, destino, quarentena, estimativa e regras), os efeitos concretos que o ensaio achou (record.action-effects:1, com o hash que a aprovação compromete; null quando não há registro legível), o código da recusa do ensaio (effects_error, ou null) e os motivos pelos quais a API não a deixa aprovar (not_approvable; vazio quando é aprovável). O campo more indica que o limite de itens ou de bytes deixou outra ação listável para depois. A versão 2 acrescenta effects, effects_hash, effects_error e more, e os motivos effects_refused, effects_unreadable e effects_not_listed. A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV2 {
    pub jsonrpc: CommandApprovalListResponseV2Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandApprovalListResponseV2Result,
}

/// `jsonrpc` in `urn:ads:command:approval.list.response:2`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalListResponseV2Jsonrpc;

impl CommandApprovalListResponseV2Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalListResponseV2Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalListResponseV2Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV2Result {
    pub approvals: CommandApprovalListResponseV2ResultApprovals,
    pub more: bool,
}

/// `result.approvals` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV2ResultApprovalsItem>")]
pub struct CommandApprovalListResponseV2ResultApprovals(
    Vec<CommandApprovalListResponseV2ResultApprovalsItem>,
);

impl CommandApprovalListResponseV2ResultApprovals {
    pub const MAX_ITEMS: usize = 1000;

    pub fn new(
        items: Vec<CommandApprovalListResponseV2ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV2ResultApprovals",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV2ResultApprovalsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommandApprovalListResponseV2ResultApprovalsItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV2ResultApprovalsItem>>
    for CommandApprovalListResponseV2ResultApprovals
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV2ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV2ResultApprovals>
    for Vec<CommandApprovalListResponseV2ResultApprovalsItem>
{
    fn from(
        value: CommandApprovalListResponseV2ResultApprovals,
    ) -> Vec<CommandApprovalListResponseV2ResultApprovalsItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV2ResultApprovals {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[]` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV2ResultApprovalsItem {
    pub action_id: CommonIdV1,
    pub approval_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub risk: CommonRiskV1,
    pub summary: CommandApprovalListResponseV2ResultApprovalsItemSummary,
    pub requested_at: CommonTimestampV1,
    pub expires_at: CommonTimestampV1,
    pub not_approvable: CommandApprovalListResponseV2ResultApprovalsItemNotApprovable,
    pub plan: RecordActionPlanV2,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_hash: Option<CommonSha256V1>,
    #[serde(deserialize_with = "support::nullable")]
    pub effects: Option<RecordActionEffectsV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_error: Option<CommandApprovalListResponseV2ResultApprovalsItemEffectsError>,
}

/// `result.approvals[].summary` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV2ResultApprovalsItemSummary(String);

impl CommandApprovalListResponseV2ResultApprovalsItemSummary {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV2ResultApprovalsItemSummary",
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

impl TryFrom<String> for CommandApprovalListResponseV2ResultApprovalsItemSummary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV2ResultApprovalsItemSummary> for String {
    fn from(value: CommandApprovalListResponseV2ResultApprovalsItemSummary) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV2ResultApprovalsItemSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV2ResultApprovalsItemSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `result.approvals[].not_approvable` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem>")]
pub struct CommandApprovalListResponseV2ResultApprovalsItemNotApprovable(
    Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem>,
);

impl CommandApprovalListResponseV2ResultApprovalsItemNotApprovable {
    pub const MAX_ITEMS: usize = 7;

    pub fn new(
        items: Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV2ResultApprovalsItemNotApprovable",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem] {
        &self.0
    }

    pub fn into_vec(
        self,
    ) -> Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem>>
    for CommandApprovalListResponseV2ResultApprovalsItemNotApprovable
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV2ResultApprovalsItemNotApprovable>
    for Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem>
{
    fn from(
        value: CommandApprovalListResponseV2ResultApprovalsItemNotApprovable,
    ) -> Vec<CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV2ResultApprovalsItemNotApprovable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[].not_approvable[]` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem {
    #[serde(rename = "estimate_unknown")]
    EstimateUnknown,
    #[serde(rename = "wildcard_target")]
    WildcardTarget,
    #[serde(rename = "definitive_recursive_delete")]
    DefinitiveRecursiveDelete,
    #[serde(rename = "restore_from_manifest")]
    RestoreFromManifest,
    #[serde(rename = "effects_refused")]
    EffectsRefused,
    #[serde(rename = "effects_unreadable")]
    EffectsUnreadable,
    #[serde(rename = "effects_not_listed")]
    EffectsNotListed,
}

impl CommandApprovalListResponseV2ResultApprovalsItemNotApprovableItem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EstimateUnknown => "estimate_unknown",
            Self::WildcardTarget => "wildcard_target",
            Self::DefinitiveRecursiveDelete => "definitive_recursive_delete",
            Self::RestoreFromManifest => "restore_from_manifest",
            Self::EffectsRefused => "effects_refused",
            Self::EffectsUnreadable => "effects_unreadable",
            Self::EffectsNotListed => "effects_not_listed",
        }
    }
}

static COMMAND_APPROVAL_LIST_RESPONSE_V2_RESULT_APPROVALS_ITEM_EFFECTS_ERROR_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `result.approvals[].effects_error` in `urn:ads:command:approval.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV2ResultApprovalsItemEffectsError(String);

impl CommandApprovalListResponseV2ResultApprovalsItemEffectsError {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV2ResultApprovalsItemEffectsError",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMAND_APPROVAL_LIST_RESPONSE_V2_RESULT_APPROVALS_ITEM_EFFECTS_ERROR_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalListResponseV2ResultApprovalsItemEffectsError {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV2ResultApprovalsItemEffectsError> for String {
    fn from(value: CommandApprovalListResponseV2ResultApprovalsItemEffectsError) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV2ResultApprovalsItemEffectsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV2ResultApprovalsItemEffectsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
