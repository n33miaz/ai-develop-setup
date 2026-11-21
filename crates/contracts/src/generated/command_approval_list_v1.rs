// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.list.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Listar aprovações pendentes. Pede ao núcleo as ações do portão que aguardam a resposta do dono, cada uma com o plano exato do ensaio. Só leitura: não muda nada no núcleo e não entra na cache de idempotência. A resposta aceita é command.approval.list.response:1; a recusa volta como command.response:2 com error.
///
/// `urn:ads:command:approval.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListV1 {
    pub jsonrpc: CommandApprovalListV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandApprovalListV1Method,
    pub params: CommandApprovalListV1Params,
}

/// `jsonrpc` in `urn:ads:command:approval.list:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalListV1Jsonrpc;

impl CommandApprovalListV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalListV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalListV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:approval.list:1`.
///
/// Always `"approval.list"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalListV1Method;

impl CommandApprovalListV1Method {
    pub const VALUE: &str = "approval.list";
}

impl Serialize for CommandApprovalListV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalListV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:approval.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandApprovalListV1ParamsMeta,
}

static COMMAND_APPROVAL_LIST_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:approval.list:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandApprovalListV1ParamsMetaFields")]
pub struct CommandApprovalListV1ParamsMeta(CommandApprovalListV1ParamsMetaFields);

/// The fields of [`CommandApprovalListV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandApprovalListV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandApprovalListV1ParamsMetaRemote>,
}

impl CommandApprovalListV1ParamsMeta {
    pub fn new(fields: CommandApprovalListV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandApprovalListV1ParamsMeta",
            &fields,
            COMMAND_APPROVAL_LIST_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandApprovalListV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandApprovalListV1ParamsMeta {
    type Target = CommandApprovalListV1ParamsMetaFields;

    fn deref(&self) -> &CommandApprovalListV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandApprovalListV1ParamsMetaFields> for CommandApprovalListV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandApprovalListV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandApprovalListV1ParamsMeta> for CommandApprovalListV1ParamsMetaFields {
    fn from(value: CommandApprovalListV1ParamsMeta) -> CommandApprovalListV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandApprovalListV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:approval.list:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalListV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandApprovalListV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:approval.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalListV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandApprovalListV1ParamsMetaRemoteSignature,
    pub nonce: CommandApprovalListV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_APPROVAL_LIST_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:approval.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListV1ParamsMetaRemoteSignature(String);

impl CommandApprovalListV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_APPROVAL_LIST_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalListV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandApprovalListV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_APPROVAL_LIST_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:approval.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalListV1ParamsMetaRemoteNonce(String);

impl CommandApprovalListV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalListV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_APPROVAL_LIST_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalListV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalListV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandApprovalListV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalListV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalListV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
