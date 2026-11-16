// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.events.subscribe.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Assinar eventos. Pede ao núcleo os eventos de um stream com seq maior que after_seq (0: tudo o que o núcleo ainda guarda), em ordem, e depois os novos, até a conexão cair. Os eventos chegam como mensagens do servidor (local-api.server-message).
///
/// `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEventsSubscribeV1 {
    pub jsonrpc: CommandEventsSubscribeV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandEventsSubscribeV1Method,
    pub params: CommandEventsSubscribeV1Params,
}

/// `jsonrpc` in `urn:ads:command:events.subscribe:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandEventsSubscribeV1Jsonrpc;

impl CommandEventsSubscribeV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandEventsSubscribeV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandEventsSubscribeV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:events.subscribe:1`.
///
/// Always `"events.subscribe"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandEventsSubscribeV1Method;

impl CommandEventsSubscribeV1Method {
    pub const VALUE: &str = "events.subscribe";
}

impl Serialize for CommandEventsSubscribeV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandEventsSubscribeV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEventsSubscribeV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandEventsSubscribeV1ParamsMeta,
    pub stream: CommandEventsSubscribeV1ParamsStream,
    pub after_seq: u64,
}

static COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:events.subscribe:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandEventsSubscribeV1ParamsMetaFields")]
pub struct CommandEventsSubscribeV1ParamsMeta(CommandEventsSubscribeV1ParamsMetaFields);

/// The fields of [`CommandEventsSubscribeV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEventsSubscribeV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandEventsSubscribeV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandEventsSubscribeV1ParamsMetaRemote>,
}

impl CommandEventsSubscribeV1ParamsMeta {
    pub fn new(fields: CommandEventsSubscribeV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandEventsSubscribeV1ParamsMeta",
            &fields,
            COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandEventsSubscribeV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandEventsSubscribeV1ParamsMeta {
    type Target = CommandEventsSubscribeV1ParamsMetaFields;

    fn deref(&self) -> &CommandEventsSubscribeV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandEventsSubscribeV1ParamsMetaFields> for CommandEventsSubscribeV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandEventsSubscribeV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandEventsSubscribeV1ParamsMeta> for CommandEventsSubscribeV1ParamsMetaFields {
    fn from(value: CommandEventsSubscribeV1ParamsMeta) -> CommandEventsSubscribeV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandEventsSubscribeV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandEventsSubscribeV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandEventsSubscribeV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEventsSubscribeV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandEventsSubscribeV1ParamsMetaRemoteSignature,
    pub nonce: CommandEventsSubscribeV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandEventsSubscribeV1ParamsMetaRemoteSignature(String);

impl CommandEventsSubscribeV1ParamsMetaRemoteSignature {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandEventsSubscribeV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == 88
            && COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandEventsSubscribeV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandEventsSubscribeV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandEventsSubscribeV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandEventsSubscribeV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandEventsSubscribeV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandEventsSubscribeV1ParamsMetaRemoteNonce(String);

impl CommandEventsSubscribeV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandEventsSubscribeV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandEventsSubscribeV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandEventsSubscribeV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandEventsSubscribeV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandEventsSubscribeV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandEventsSubscribeV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_STREAM_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");

/// `params.stream` in `urn:ads:command:events.subscribe:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandEventsSubscribeV1ParamsStream(String);

impl CommandEventsSubscribeV1ParamsStream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandEventsSubscribeV1ParamsStream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMAND_EVENTS_SUBSCRIBE_V1_PARAMS_STREAM_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandEventsSubscribeV1ParamsStream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandEventsSubscribeV1ParamsStream> for String {
    fn from(value: CommandEventsSubscribeV1ParamsStream) -> String {
        value.0
    }
}

impl fmt::Display for CommandEventsSubscribeV1ParamsStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandEventsSubscribeV1ParamsStream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
