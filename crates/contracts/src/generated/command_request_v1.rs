// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.request.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Envelope de comando. Comando JSON-RPC 2.0 enviado ao núcleo. Comando é pedido, não fato; a chave de idempotência evita executar duas vezes, e a origem remota exige assinatura do aparelho, nonce, contador e validade.
///
/// `urn:ads:command:request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequestV1 {
    pub jsonrpc: CommandRequestV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandRequestV1Method,
    pub params: CommandRequestV1Params,
}

/// `jsonrpc` in `urn:ads:command:request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandRequestV1Jsonrpc;

impl CommandRequestV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandRequestV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandRequestV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static COMMAND_REQUEST_V1_METHOD_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$");

/// `method` in `urn:ads:command:request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandRequestV1Method(String);

impl CommandRequestV1Method {
    pub const MAX_CHARS: usize = 64;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommandRequestV1Method", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= Self::MAX_CHARS && COMMAND_REQUEST_V1_METHOD_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandRequestV1Method {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandRequestV1Method> for String {
    fn from(value: CommandRequestV1Method) -> String {
        value.0
    }
}

impl fmt::Display for CommandRequestV1Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandRequestV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `params` in `urn:ads:command:request:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "CommandRequestV1ParamsFields")]
pub struct CommandRequestV1Params(CommandRequestV1ParamsFields);

/// The fields of [`CommandRequestV1Params`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRequestV1ParamsFields {
    #[serde(rename = "_meta")]
    pub meta: CommandRequestV1ParamsMeta,
    /// What the Schema does not name.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl CommandRequestV1Params {
    pub fn new(fields: CommandRequestV1ParamsFields) -> Result<Self, ContractError> {
        support::check_extra("CommandRequestV1Params", &fields.extra, &["_meta"])?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandRequestV1ParamsFields {
        self.0
    }
}

impl Deref for CommandRequestV1Params {
    type Target = CommandRequestV1ParamsFields;

    fn deref(&self) -> &CommandRequestV1ParamsFields {
        &self.0
    }
}

impl TryFrom<CommandRequestV1ParamsFields> for CommandRequestV1Params {
    type Error = ContractError;

    fn try_from(fields: CommandRequestV1ParamsFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandRequestV1Params> for CommandRequestV1ParamsFields {
    fn from(value: CommandRequestV1Params) -> CommandRequestV1ParamsFields {
        value.0
    }
}

impl Serialize for CommandRequestV1Params {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static COMMAND_REQUEST_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:request:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandRequestV1ParamsMetaFields")]
pub struct CommandRequestV1ParamsMeta(CommandRequestV1ParamsMetaFields);

/// The fields of [`CommandRequestV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequestV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandRequestV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandRequestV1ParamsMetaRemote>,
}

impl CommandRequestV1ParamsMeta {
    pub fn new(fields: CommandRequestV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandRequestV1ParamsMeta",
            &fields,
            COMMAND_REQUEST_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandRequestV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandRequestV1ParamsMeta {
    type Target = CommandRequestV1ParamsMetaFields;

    fn deref(&self) -> &CommandRequestV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandRequestV1ParamsMetaFields> for CommandRequestV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandRequestV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandRequestV1ParamsMeta> for CommandRequestV1ParamsMetaFields {
    fn from(value: CommandRequestV1ParamsMeta) -> CommandRequestV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandRequestV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:request:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandRequestV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandRequestV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequestV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandRequestV1ParamsMetaRemoteSignature,
    pub nonce: CommandRequestV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_REQUEST_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandRequestV1ParamsMetaRemoteSignature(String);

impl CommandRequestV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandRequestV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_REQUEST_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandRequestV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandRequestV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandRequestV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandRequestV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandRequestV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_REQUEST_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandRequestV1ParamsMetaRemoteNonce(String);

impl CommandRequestV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandRequestV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_REQUEST_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandRequestV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandRequestV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandRequestV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandRequestV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandRequestV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
