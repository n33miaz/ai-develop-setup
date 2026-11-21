// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.stop.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Parar a fila. O dono para a fila. Parar expressa o estado desejado e é sempre aceito: o núcleo não consulta a cache de idempotência, então repetir um queue.stop antigo depois de religar a fila a para de novo, e uma chave já usada por outro comando não impede a parada. A resposta é command.response:2 com replayed true quando a fila já estava parada (nada foi gravado) e false quando esta parada a desligou e anexou queue.stopped com reason owner. A fila para antes da gravação; se a gravação falhar, a resposta é INTERNAL_ERROR com a fila parada, e o evento é gravado antes da próxima mudança feita pelo mesmo processo, com o id deste comando em cause.
///
/// `urn:ads:command:queue.stop:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStopV1 {
    pub jsonrpc: CommandQueueStopV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandQueueStopV1Method,
    pub params: CommandQueueStopV1Params,
}

/// `jsonrpc` in `urn:ads:command:queue.stop:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueStopV1Jsonrpc;

impl CommandQueueStopV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueStopV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueStopV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:queue.stop:1`.
///
/// Always `"queue.stop"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueStopV1Method;

impl CommandQueueStopV1Method {
    pub const VALUE: &str = "queue.stop";
}

impl Serialize for CommandQueueStopV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueStopV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:queue.stop:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStopV1Params {
    pub queue_id: CommonIdV1,
    #[serde(rename = "_meta")]
    pub meta: CommandQueueStopV1ParamsMeta,
}

static COMMAND_QUEUE_STOP_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:queue.stop:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandQueueStopV1ParamsMetaFields")]
pub struct CommandQueueStopV1ParamsMeta(CommandQueueStopV1ParamsMetaFields);

/// The fields of [`CommandQueueStopV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStopV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandQueueStopV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandQueueStopV1ParamsMetaRemote>,
}

impl CommandQueueStopV1ParamsMeta {
    pub fn new(fields: CommandQueueStopV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandQueueStopV1ParamsMeta",
            &fields,
            COMMAND_QUEUE_STOP_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandQueueStopV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandQueueStopV1ParamsMeta {
    type Target = CommandQueueStopV1ParamsMetaFields;

    fn deref(&self) -> &CommandQueueStopV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandQueueStopV1ParamsMetaFields> for CommandQueueStopV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandQueueStopV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandQueueStopV1ParamsMeta> for CommandQueueStopV1ParamsMetaFields {
    fn from(value: CommandQueueStopV1ParamsMeta) -> CommandQueueStopV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandQueueStopV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:queue.stop:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandQueueStopV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandQueueStopV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:queue.stop:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueStopV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandQueueStopV1ParamsMetaRemoteSignature,
    pub nonce: CommandQueueStopV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_QUEUE_STOP_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:queue.stop:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueStopV1ParamsMetaRemoteSignature(String);

impl CommandQueueStopV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueStopV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_QUEUE_STOP_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueStopV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueStopV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandQueueStopV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueStopV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueStopV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_QUEUE_STOP_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:queue.stop:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandQueueStopV1ParamsMetaRemoteNonce(String);

impl CommandQueueStopV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandQueueStopV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_QUEUE_STOP_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandQueueStopV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandQueueStopV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandQueueStopV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandQueueStopV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandQueueStopV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
