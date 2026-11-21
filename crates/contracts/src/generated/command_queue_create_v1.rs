// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.create.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Criar a fila. O dono cria a fila parada pela interface local com a política padrão. A criação anexa queue.created ao log de auditoria e recusa uma fila existente (QUEUE_ALREADY_CREATED). A resposta é command.response:2; o fato fica no evento.
///
/// `urn:ads:command:queue.create:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueCreateV1 {
    pub jsonrpc: CommandQueueCreateV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandQueueCreateV1Method,
    pub params: CommandQueueCreateV1Params,
}

/// `jsonrpc` in `urn:ads:command:queue.create:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueCreateV1Jsonrpc;

impl CommandQueueCreateV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueCreateV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueCreateV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:queue.create:1`.
///
/// Always `"queue.create"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueCreateV1Method;

impl CommandQueueCreateV1Method {
    pub const VALUE: &str = "queue.create";
}

impl Serialize for CommandQueueCreateV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueCreateV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:queue.create:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueCreateV1Params {
    pub queue_id: CommonIdV1,
    #[serde(rename = "_meta")]
    pub meta: CommandQueueCreateV1ParamsMeta,
}

static COMMAND_QUEUE_CREATE_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:queue.create:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandQueueCreateV1ParamsMetaFields")]
pub struct CommandQueueCreateV1ParamsMeta(CommandQueueCreateV1ParamsMetaFields);

/// The fields of [`CommandQueueCreateV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueCreateV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandQueueCreateV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandQueueCreateV1ParamsMetaRemote>,
}

impl CommandQueueCreateV1ParamsMeta {
    pub fn new(fields: CommandQueueCreateV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandQueueCreateV1ParamsMeta",
            &fields,
            COMMAND_QUEUE_CREATE_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandQueueCreateV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandQueueCreateV1ParamsMeta {
    type Target = CommandQueueCreateV1ParamsMetaFields;

    fn deref(&self) -> &CommandQueueCreateV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandQueueCreateV1ParamsMetaFields> for CommandQueueCreateV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandQueueCreateV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandQueueCreateV1ParamsMeta> for CommandQueueCreateV1ParamsMetaFields {
    fn from(value: CommandQueueCreateV1ParamsMeta) -> CommandQueueCreateV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandQueueCreateV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:queue.create:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandQueueCreateV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandQueueCreateV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:queue.create:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueCreateV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandQueueCreateV1ParamsMetaRemoteSignature,
    pub nonce: CommandQueueCreateV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_QUEUE_CREATE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:queue.create:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueCreateV1ParamsMetaRemoteSignature(String);

impl CommandQueueCreateV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueCreateV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_QUEUE_CREATE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueCreateV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueCreateV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandQueueCreateV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueCreateV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueCreateV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_QUEUE_CREATE_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:queue.create:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueCreateV1ParamsMetaRemoteNonce(String);

impl CommandQueueCreateV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueCreateV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_QUEUE_CREATE_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueCreateV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueCreateV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandQueueCreateV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueCreateV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueCreateV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
