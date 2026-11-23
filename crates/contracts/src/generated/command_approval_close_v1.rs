// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.close.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Fechar um pedido de aprovação ilegível. Recuperação: o dono fecha, pelo id da ação, os pedidos de aprovação pendentes de uma ação cujo histórico o núcleo não consegue ler (o mesmo id que o log do daemon registra quando deixa o pedido fora de approval.list). A decisão é negar (deny) ou expirar (expire); nunca aprova e nunca emite licença. Um pedido com histórico legível é recusado com APPROVAL_READABLE e continua sendo respondido por approval.answer. O dono e o aparelho vêm do núcleo, nunca do pedido. A resposta é command.response:2.
///
/// `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalCloseV1 {
    pub jsonrpc: CommandApprovalCloseV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandApprovalCloseV1Method,
    pub params: CommandApprovalCloseV1Params,
}

/// `jsonrpc` in `urn:ads:command:approval.close:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalCloseV1Jsonrpc;

impl CommandApprovalCloseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalCloseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalCloseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:approval.close:1`.
///
/// Always `"approval.close"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalCloseV1Method;

impl CommandApprovalCloseV1Method {
    pub const VALUE: &str = "approval.close";
}

impl Serialize for CommandApprovalCloseV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalCloseV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalCloseV1Params {
    pub action_id: CommonIdV1,
    pub decision: CommandApprovalCloseV1ParamsDecision,
    #[serde(rename = "_meta")]
    pub meta: CommandApprovalCloseV1ParamsMeta,
}

/// `params.decision` in `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalCloseV1ParamsDecision {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "expire")]
    Expire,
}

impl CommandApprovalCloseV1ParamsDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Expire => "expire",
        }
    }
}

static COMMAND_APPROVAL_CLOSE_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
    "#/properties/params/properties/_meta/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[("origin", Rule::Const(Literal::Str("remote")))]),
            Rule::Required(&["origin"]),
        ]),
        then: Some(&Rule::Required(&["remote"])),
        otherwise: Some(&Rule::Not(&Rule::Required(&["remote"]))),
    },
)];

/// `params._meta` in `urn:ads:command:approval.close:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandApprovalCloseV1ParamsMetaFields")]
pub struct CommandApprovalCloseV1ParamsMeta(CommandApprovalCloseV1ParamsMetaFields);

/// The fields of [`CommandApprovalCloseV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalCloseV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandApprovalCloseV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandApprovalCloseV1ParamsMetaRemote>,
}

impl CommandApprovalCloseV1ParamsMeta {
    pub fn new(fields: CommandApprovalCloseV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandApprovalCloseV1ParamsMeta",
            &fields,
            COMMAND_APPROVAL_CLOSE_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandApprovalCloseV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandApprovalCloseV1ParamsMeta {
    type Target = CommandApprovalCloseV1ParamsMetaFields;

    fn deref(&self) -> &CommandApprovalCloseV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandApprovalCloseV1ParamsMetaFields> for CommandApprovalCloseV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandApprovalCloseV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandApprovalCloseV1ParamsMeta> for CommandApprovalCloseV1ParamsMetaFields {
    fn from(value: CommandApprovalCloseV1ParamsMeta) -> CommandApprovalCloseV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandApprovalCloseV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalCloseV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandApprovalCloseV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalCloseV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandApprovalCloseV1ParamsMetaRemoteSignature,
    pub nonce: CommandApprovalCloseV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_APPROVAL_CLOSE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalCloseV1ParamsMetaRemoteSignature(String);

impl CommandApprovalCloseV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalCloseV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_APPROVAL_CLOSE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalCloseV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalCloseV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandApprovalCloseV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalCloseV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalCloseV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_APPROVAL_CLOSE_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:approval.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalCloseV1ParamsMetaRemoteNonce(String);

impl CommandApprovalCloseV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalCloseV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_APPROVAL_CLOSE_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalCloseV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalCloseV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandApprovalCloseV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalCloseV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalCloseV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
