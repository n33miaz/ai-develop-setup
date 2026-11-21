// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.list.response.v3.schema.json. Não edite à mão.

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

/// Resposta de approval.list. Resposta JSON-RPC 2.0 aceita de approval.list: as ações que aguardam a resposta do dono, da mais antiga à mais nova, cada uma com a aprovação pedida, o registro do plano como o portão o julgou (alvos como pedidos, destino, quarentena, estimativa e regras), os efeitos concretos que o ensaio achou (record.action-effects:1, com o hash que a aprovação compromete; null quando não há registro legível), o código da recusa do ensaio (effects_error, ou null) e os motivos pelos quais a API não a deixa aprovar (not_approvable; vazio quando é aprovável). O campo more indica que o limite de itens ou de bytes deixou outra ação listável para depois. O campo unreadable conta os pedidos pendentes com histórico ilegível que a listagem deixou de fora até onde chegou (todos quando more é false); os ids ficam no log do daemon e approval.close os fecha. A versão 2 acrescentou effects, effects_hash, effects_error e more, e os motivos effects_refused, effects_unreadable e effects_not_listed; a versão 3 acrescenta unreadable. A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV3 {
    pub jsonrpc: CommandApprovalListResponseV3Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandApprovalListResponseV3Result,
}

/// `jsonrpc` in `urn:ads:command:approval.list.response:3`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalListResponseV3Jsonrpc;

impl CommandApprovalListResponseV3Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalListResponseV3Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalListResponseV3Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV3Result {
    pub approvals: CommandApprovalListResponseV3ResultApprovals,
    pub more: bool,
    pub unreadable: u64,
}

/// `result.approvals` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV3ResultApprovalsItem>")]
pub struct CommandApprovalListResponseV3ResultApprovals(
    Vec<CommandApprovalListResponseV3ResultApprovalsItem>,
);

impl CommandApprovalListResponseV3ResultApprovals {
    pub const MAX_ITEMS: usize = 1000;

    pub fn new(
        items: Vec<CommandApprovalListResponseV3ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV3ResultApprovals",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV3ResultApprovalsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommandApprovalListResponseV3ResultApprovalsItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV3ResultApprovalsItem>>
    for CommandApprovalListResponseV3ResultApprovals
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV3ResultApprovalsItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV3ResultApprovals>
    for Vec<CommandApprovalListResponseV3ResultApprovalsItem>
{
    fn from(
        value: CommandApprovalListResponseV3ResultApprovals,
    ) -> Vec<CommandApprovalListResponseV3ResultApprovalsItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV3ResultApprovals {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[]` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListResponseV3ResultApprovalsItem {
    pub action_id: CommonIdV1,
    pub approval_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub risk: CommonRiskV1,
    pub summary: CommandApprovalListResponseV3ResultApprovalsItemSummary,
    pub requested_at: CommonTimestampV1,
    pub expires_at: CommonTimestampV1,
    pub not_approvable: CommandApprovalListResponseV3ResultApprovalsItemNotApprovable,
    pub plan: RecordActionPlanV2,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_hash: Option<CommonSha256V1>,
    #[serde(deserialize_with = "support::nullable")]
    pub effects: Option<RecordActionEffectsV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_error: Option<CommandApprovalListResponseV3ResultApprovalsItemEffectsError>,
}

/// `result.approvals[].summary` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV3ResultApprovalsItemSummary(String);

impl CommandApprovalListResponseV3ResultApprovalsItemSummary {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV3ResultApprovalsItemSummary",
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

impl TryFrom<String> for CommandApprovalListResponseV3ResultApprovalsItemSummary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV3ResultApprovalsItemSummary> for String {
    fn from(value: CommandApprovalListResponseV3ResultApprovalsItemSummary) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV3ResultApprovalsItemSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV3ResultApprovalsItemSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `result.approvals[].not_approvable` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem>")]
pub struct CommandApprovalListResponseV3ResultApprovalsItemNotApprovable(
    Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem>,
);

impl CommandApprovalListResponseV3ResultApprovalsItemNotApprovable {
    pub const MAX_ITEMS: usize = 7;

    pub fn new(
        items: Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV3ResultApprovalsItemNotApprovable",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem] {
        &self.0
    }

    pub fn into_vec(
        self,
    ) -> Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem> {
        self.0
    }
}

impl TryFrom<Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem>>
    for CommandApprovalListResponseV3ResultApprovalsItemNotApprovable
{
    type Error = ContractError;

    fn try_from(
        items: Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandApprovalListResponseV3ResultApprovalsItemNotApprovable>
    for Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem>
{
    fn from(
        value: CommandApprovalListResponseV3ResultApprovalsItemNotApprovable,
    ) -> Vec<CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem> {
        value.0
    }
}

impl Serialize for CommandApprovalListResponseV3ResultApprovalsItemNotApprovable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.approvals[].not_approvable[]` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem {
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

impl CommandApprovalListResponseV3ResultApprovalsItemNotApprovableItem {
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

static COMMAND_APPROVAL_LIST_RESPONSE_V3_RESULT_APPROVALS_ITEM_EFFECTS_ERROR_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `result.approvals[].effects_error` in `urn:ads:command:approval.list.response:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListResponseV3ResultApprovalsItemEffectsError(String);

impl CommandApprovalListResponseV3ResultApprovalsItemEffectsError {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListResponseV3ResultApprovalsItemEffectsError",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMAND_APPROVAL_LIST_RESPONSE_V3_RESULT_APPROVALS_ITEM_EFFECTS_ERROR_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalListResponseV3ResultApprovalsItemEffectsError {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListResponseV3ResultApprovalsItemEffectsError> for String {
    fn from(value: CommandApprovalListResponseV3ResultApprovalsItemEffectsError) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListResponseV3ResultApprovalsItemEffectsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListResponseV3ResultApprovalsItemEffectsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
