// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.list.response.v4.schema.json. Não edite à mão.

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

/// Resposta de approval.list. Resposta JSON-RPC 2.0 aceita de approval.list: as ações que aguardam a resposta do dono, da mais antiga à mais nova, cada uma com a aprovação pedida, o registro do plano como o portão o julgou (alvos como pedidos, destino, quarentena, estimativa e regras), os efeitos concretos que o ensaio achou (record.action-effects:1, com o hash que a aprovação compromete; null quando não há registro legível), o código da recusa do ensaio (effects_error, ou null) e os motivos pelos quais a API não a deixa aprovar (not_approvable; vazio quando é aprovável). O campo more indica que o limite de itens ou de bytes deixou outra ação listável para depois. O campo unreadable conta os pedidos pendentes com histórico ilegível que a listagem deixou de fora até onde chegou (todos quando more é false); os ids ficam no log do daemon e approval.close os fecha. A versão 2 acrescentou effects, effects_hash, effects_error e more, e os motivos effects_refused, effects_unreadable e effects_not_listed; a versão 3 acrescentou unreadable e a versão 4 acrescenta policy_changed quando a política vigente impede a aprovação. A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV4 {
    pub jsonrpc: CommandApprovalListResponseV4Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandApprovalListResponseV4Result,
}

/// `jsonrpc` in `urn:ads:command:approval.list.response:4`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalListResponseV4Jsonrpc;

impl CommandApprovalListResponseV4Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalListResponseV4Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalListResponseV4Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV4Result {
    pub approvals: CommandApprovalListResponseV4ResultApprovals,
    pub more: bool,
    pub unreadable: u64,
}

/// `result.approvals` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV4ResultApprovalsItem>")]
pub struct CommandApprovalListResponseV4ResultApprovals(
    Vec<CommandApprovalListResponseV4ResultApprovalsItem>,
);

impl CommandApprovalListResponseV4ResultApprovals {
    pub const MAX_ITEMS: usize = 1000;

    pub fn new(
        items: Vec<CommandApprovalListResponseV4ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV4ResultApprovals",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV4ResultApprovalsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommandApprovalListResponseV4ResultApprovalsItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV4ResultApprovalsItem>>
    for CommandApprovalListResponseV4ResultApprovals
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV4ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV4ResultApprovals>
    for Vec<CommandApprovalListResponseV4ResultApprovalsItem>
{
    fn from(
        value: CommandApprovalListResponseV4ResultApprovals,
    ) -> Vec<CommandApprovalListResponseV4ResultApprovalsItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV4ResultApprovals {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[]` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV4ResultApprovalsItem {
    pub action_id: CommonIdV1,
    pub approval_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub risk: CommonRiskV1,
    pub summary: CommandApprovalListResponseV4ResultApprovalsItemSummary,
    pub requested_at: CommonTimestampV1,
    pub expires_at: CommonTimestampV1,
    pub not_approvable: CommandApprovalListResponseV4ResultApprovalsItemNotApprovable,
    pub plan: RecordActionPlanV2,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_hash: Option<CommonSha256V1>,
    #[serde(deserialize_with = "support::nullable")]
    pub effects: Option<RecordActionEffectsV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_error: Option<CommandApprovalListResponseV4ResultApprovalsItemEffectsError>,
}

/// `result.approvals[].summary` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV4ResultApprovalsItemSummary(String);

impl CommandApprovalListResponseV4ResultApprovalsItemSummary {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV4ResultApprovalsItemSummary",
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

impl TryFrom<String> for CommandApprovalListResponseV4ResultApprovalsItemSummary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV4ResultApprovalsItemSummary> for String {
    fn from(value: CommandApprovalListResponseV4ResultApprovalsItemSummary) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV4ResultApprovalsItemSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV4ResultApprovalsItemSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `result.approvals[].not_approvable` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem>")]
pub struct CommandApprovalListResponseV4ResultApprovalsItemNotApprovable(
    Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem>,
);

impl CommandApprovalListResponseV4ResultApprovalsItemNotApprovable {
    pub const MAX_ITEMS: usize = 8;

    pub fn new(
        items: Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV4ResultApprovalsItemNotApprovable",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem] {
        &self.0
    }

    pub fn into_vec(
        self,
    ) -> Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem>>
    for CommandApprovalListResponseV4ResultApprovalsItemNotApprovable
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV4ResultApprovalsItemNotApprovable>
    for Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem>
{
    fn from(
        value: CommandApprovalListResponseV4ResultApprovalsItemNotApprovable,
    ) -> Vec<CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV4ResultApprovalsItemNotApprovable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[].not_approvable[]` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem {
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
    #[serde(rename = "policy_changed")]
    PolicyChanged,
}

impl CommandApprovalListResponseV4ResultApprovalsItemNotApprovableItem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EstimateUnknown => "estimate_unknown",
            Self::WildcardTarget => "wildcard_target",
            Self::DefinitiveRecursiveDelete => "definitive_recursive_delete",
            Self::RestoreFromManifest => "restore_from_manifest",
            Self::EffectsRefused => "effects_refused",
            Self::EffectsUnreadable => "effects_unreadable",
            Self::EffectsNotListed => "effects_not_listed",
            Self::PolicyChanged => "policy_changed",
        }
    }
}

static COMMAND_APPROVAL_LIST_RESPONSE_V4_RESULT_APPROVALS_ITEM_EFFECTS_ERROR_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `result.approvals[].effects_error` in `urn:ads:command:approval.list.response:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV4ResultApprovalsItemEffectsError(String);

impl CommandApprovalListResponseV4ResultApprovalsItemEffectsError {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV4ResultApprovalsItemEffectsError",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMAND_APPROVAL_LIST_RESPONSE_V4_RESULT_APPROVALS_ITEM_EFFECTS_ERROR_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalListResponseV4ResultApprovalsItemEffectsError {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV4ResultApprovalsItemEffectsError> for String {
    fn from(value: CommandApprovalListResponseV4ResultApprovalsItemEffectsError) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV4ResultApprovalsItemEffectsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV4ResultApprovalsItemEffectsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
