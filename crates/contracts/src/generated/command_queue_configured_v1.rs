// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.configured.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Informar a fila configurada. Pede ao núcleo o id da fila de tarefas que a configuração local dele serve, criada ou não. Só leitura: não muda nada no núcleo e não entra na cache de idempotência. A resposta aceita é command.queue.configured.response:1; sem controles de fila, a recusa volta como command.response:2 com QUEUE_NOT_CONFIGURED.
///
/// `urn:ads:command:queue.configured:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueConfiguredV1 {
    pub jsonrpc: CommandQueueConfiguredV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandQueueConfiguredV1Method,
    pub params: CommandQueueConfiguredV1Params,
}

/// `jsonrpc` in `urn:ads:command:queue.configured:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueConfiguredV1Jsonrpc;

impl CommandQueueConfiguredV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueConfiguredV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueConfiguredV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:queue.configured:1`.
///
/// Always `"queue.configured"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueConfiguredV1Method;

impl CommandQueueConfiguredV1Method {
    pub const VALUE: &str = "queue.configured";
}

impl Serialize for CommandQueueConfiguredV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueConfiguredV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:queue.configured:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueConfiguredV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandQueueConfiguredV1ParamsMeta,
}

static COMMAND_QUEUE_CONFIGURED_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:queue.configured:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandQueueConfiguredV1ParamsMetaFields")]
pub struct CommandQueueConfiguredV1ParamsMeta(CommandQueueConfiguredV1ParamsMetaFields);

/// The fields of [`CommandQueueConfiguredV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueConfiguredV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandQueueConfiguredV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandQueueConfiguredV1ParamsMetaRemote>,
}

impl CommandQueueConfiguredV1ParamsMeta {
    pub fn new(fields: CommandQueueConfiguredV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandQueueConfiguredV1ParamsMeta",
            &fields,
            COMMAND_QUEUE_CONFIGURED_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandQueueConfiguredV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandQueueConfiguredV1ParamsMeta {
    type Target = CommandQueueConfiguredV1ParamsMetaFields;

    fn deref(&self) -> &CommandQueueConfiguredV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandQueueConfiguredV1ParamsMetaFields> for CommandQueueConfiguredV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandQueueConfiguredV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandQueueConfiguredV1ParamsMeta> for CommandQueueConfiguredV1ParamsMetaFields {
    fn from(value: CommandQueueConfiguredV1ParamsMeta) -> CommandQueueConfiguredV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandQueueConfiguredV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:queue.configured:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandQueueConfiguredV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandQueueConfiguredV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:queue.configured:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueConfiguredV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandQueueConfiguredV1ParamsMetaRemoteSignature,
    pub nonce: CommandQueueConfiguredV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_QUEUE_CONFIGURED_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:queue.configured:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueConfiguredV1ParamsMetaRemoteSignature(String);

impl CommandQueueConfiguredV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueConfiguredV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_QUEUE_CONFIGURED_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueConfiguredV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueConfiguredV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandQueueConfiguredV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueConfiguredV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueConfiguredV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_QUEUE_CONFIGURED_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:queue.configured:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueConfiguredV1ParamsMetaRemoteNonce(String);

impl CommandQueueConfiguredV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueConfiguredV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_QUEUE_CONFIGURED_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueConfiguredV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueConfiguredV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandQueueConfiguredV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueConfiguredV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueConfiguredV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
