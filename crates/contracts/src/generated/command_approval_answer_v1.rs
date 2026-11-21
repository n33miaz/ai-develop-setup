// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.approval.answer.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Responder uma aprovação. Aprova ou nega uma ação que aguarda o dono, pelo id da ação e da aprovação e pelo hash do plano que o dono viu. O dono e o aparelho vêm do núcleo, nunca do pedido. A resposta é command.response:2.
///
/// `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalAnswerV1 {
    pub jsonrpc: CommandApprovalAnswerV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandApprovalAnswerV1Method,
    pub params: CommandApprovalAnswerV1Params,
}

/// `jsonrpc` in `urn:ads:command:approval.answer:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalAnswerV1Jsonrpc;

impl CommandApprovalAnswerV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandApprovalAnswerV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalAnswerV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:approval.answer:1`.
///
/// Always `"approval.answer"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandApprovalAnswerV1Method;

impl CommandApprovalAnswerV1Method {
    pub const VALUE: &str = "approval.answer";
}

impl Serialize for CommandApprovalAnswerV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandApprovalAnswerV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalAnswerV1Params {
    pub action_id: CommonIdV1,
    pub approval_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub decision: CommandApprovalAnswerV1ParamsDecision,
    #[serde(rename = "_meta")]
    pub meta: CommandApprovalAnswerV1ParamsMeta,
}

/// `params.decision` in `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalAnswerV1ParamsDecision {
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "deny")]
    Deny,
}

impl CommandApprovalAnswerV1ParamsDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }
}

static COMMAND_APPROVAL_ANSWER_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:approval.answer:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandApprovalAnswerV1ParamsMetaFields")]
pub struct CommandApprovalAnswerV1ParamsMeta(CommandApprovalAnswerV1ParamsMetaFields);

/// The fields of [`CommandApprovalAnswerV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalAnswerV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandApprovalAnswerV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandApprovalAnswerV1ParamsMetaRemote>,
}

impl CommandApprovalAnswerV1ParamsMeta {
    pub fn new(fields: CommandApprovalAnswerV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandApprovalAnswerV1ParamsMeta",
            &fields,
            COMMAND_APPROVAL_ANSWER_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandApprovalAnswerV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandApprovalAnswerV1ParamsMeta {
    type Target = CommandApprovalAnswerV1ParamsMetaFields;

    fn deref(&self) -> &CommandApprovalAnswerV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandApprovalAnswerV1ParamsMetaFields> for CommandApprovalAnswerV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandApprovalAnswerV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandApprovalAnswerV1ParamsMeta> for CommandApprovalAnswerV1ParamsMetaFields {
    fn from(value: CommandApprovalAnswerV1ParamsMeta) -> CommandApprovalAnswerV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandApprovalAnswerV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandApprovalAnswerV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandApprovalAnswerV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandApprovalAnswerV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandApprovalAnswerV1ParamsMetaRemoteSignature,
    pub nonce: CommandApprovalAnswerV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_APPROVAL_ANSWER_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalAnswerV1ParamsMetaRemoteSignature(String);

impl CommandApprovalAnswerV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalAnswerV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_APPROVAL_ANSWER_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalAnswerV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalAnswerV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandApprovalAnswerV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalAnswerV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalAnswerV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_APPROVAL_ANSWER_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:approval.answer:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandApprovalAnswerV1ParamsMetaRemoteNonce(String);

impl CommandApprovalAnswerV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandApprovalAnswerV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_APPROVAL_ANSWER_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandApprovalAnswerV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandApprovalAnswerV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandApprovalAnswerV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandApprovalAnswerV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandApprovalAnswerV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
