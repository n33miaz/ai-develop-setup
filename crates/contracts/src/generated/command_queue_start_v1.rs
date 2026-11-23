// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.start.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Ligar a fila. O dono liga a fila pela interface local. O núcleo confere o dono, recusa a fila já ligada (QUEUE_ALREADY_ENABLED) e anexa queue.enabled ao log de auditoria antes de passar a admitir tarefas. A resposta é command.response:2; o fato fica no evento.
///
/// `urn:ads:command:queue.start:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStartV1 {
    pub jsonrpc: CommandQueueStartV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandQueueStartV1Method,
    pub params: CommandQueueStartV1Params,
}

/// `jsonrpc` in `urn:ads:command:queue.start:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueStartV1Jsonrpc;

impl CommandQueueStartV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueStartV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueStartV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:queue.start:1`.
///
/// Always `"queue.start"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueStartV1Method;

impl CommandQueueStartV1Method {
    pub const VALUE: &str = "queue.start";
}

impl Serialize for CommandQueueStartV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueStartV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:queue.start:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStartV1Params {
    pub queue_id: CommonIdV1,
    #[serde(rename = "_meta")]
    pub meta: CommandQueueStartV1ParamsMeta,
}

static COMMAND_QUEUE_START_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:queue.start:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandQueueStartV1ParamsMetaFields")]
pub struct CommandQueueStartV1ParamsMeta(CommandQueueStartV1ParamsMetaFields);

/// The fields of [`CommandQueueStartV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStartV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandQueueStartV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandQueueStartV1ParamsMetaRemote>,
}

impl CommandQueueStartV1ParamsMeta {
    pub fn new(fields: CommandQueueStartV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandQueueStartV1ParamsMeta",
            &fields,
            COMMAND_QUEUE_START_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandQueueStartV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandQueueStartV1ParamsMeta {
    type Target = CommandQueueStartV1ParamsMetaFields;

    fn deref(&self) -> &CommandQueueStartV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandQueueStartV1ParamsMetaFields> for CommandQueueStartV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandQueueStartV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandQueueStartV1ParamsMeta> for CommandQueueStartV1ParamsMetaFields {
    fn from(value: CommandQueueStartV1ParamsMeta) -> CommandQueueStartV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandQueueStartV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:queue.start:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandQueueStartV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandQueueStartV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:queue.start:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStartV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandQueueStartV1ParamsMetaRemoteSignature,
    pub nonce: CommandQueueStartV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_QUEUE_START_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:queue.start:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueStartV1ParamsMetaRemoteSignature(String);

impl CommandQueueStartV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueStartV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_QUEUE_START_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueStartV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueStartV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandQueueStartV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueStartV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueStartV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_QUEUE_START_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:queue.start:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueStartV1ParamsMetaRemoteNonce(String);

impl CommandQueueStartV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueStartV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_QUEUE_START_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueStartV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueStartV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandQueueStartV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueStartV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueStartV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
