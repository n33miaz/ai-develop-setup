// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.auth.hello.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Autenticação da API local. Primeira mensagem de toda conexão da interface com a API local do núcleo: leva o token aleatório da execução, que o núcleo compara em tempo constante. Antes dela nenhum outro comando é aceito. O token nunca vai em query string, disco ou log.
///
/// `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandAuthHelloV1 {
    pub jsonrpc: CommandAuthHelloV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandAuthHelloV1Method,
    pub params: CommandAuthHelloV1Params,
}

/// `jsonrpc` in `urn:ads:command:auth.hello:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandAuthHelloV1Jsonrpc;

impl CommandAuthHelloV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandAuthHelloV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandAuthHelloV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:auth.hello:1`.
///
/// Always `"auth.hello"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandAuthHelloV1Method;

impl CommandAuthHelloV1Method {
    pub const VALUE: &str = "auth.hello";
}

impl Serialize for CommandAuthHelloV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandAuthHelloV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandAuthHelloV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandAuthHelloV1ParamsMeta,
    pub token: CommandAuthHelloV1ParamsToken,
}

static COMMAND_AUTH_HELLO_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:auth.hello:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandAuthHelloV1ParamsMetaFields")]
pub struct CommandAuthHelloV1ParamsMeta(CommandAuthHelloV1ParamsMetaFields);

/// The fields of [`CommandAuthHelloV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandAuthHelloV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandAuthHelloV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandAuthHelloV1ParamsMetaRemote>,
}

impl CommandAuthHelloV1ParamsMeta {
    pub fn new(fields: CommandAuthHelloV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandAuthHelloV1ParamsMeta",
            &fields,
            COMMAND_AUTH_HELLO_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandAuthHelloV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandAuthHelloV1ParamsMeta {
    type Target = CommandAuthHelloV1ParamsMetaFields;

    fn deref(&self) -> &CommandAuthHelloV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandAuthHelloV1ParamsMetaFields> for CommandAuthHelloV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandAuthHelloV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandAuthHelloV1ParamsMeta> for CommandAuthHelloV1ParamsMetaFields {
    fn from(value: CommandAuthHelloV1ParamsMeta) -> CommandAuthHelloV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandAuthHelloV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandAuthHelloV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandAuthHelloV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandAuthHelloV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandAuthHelloV1ParamsMetaRemoteSignature,
    pub nonce: CommandAuthHelloV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_AUTH_HELLO_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandAuthHelloV1ParamsMetaRemoteSignature(String);

impl CommandAuthHelloV1ParamsMetaRemoteSignature {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandAuthHelloV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == 88 && COMMAND_AUTH_HELLO_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandAuthHelloV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandAuthHelloV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandAuthHelloV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandAuthHelloV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandAuthHelloV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_AUTH_HELLO_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandAuthHelloV1ParamsMetaRemoteNonce(String);

impl CommandAuthHelloV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandAuthHelloV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_AUTH_HELLO_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandAuthHelloV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandAuthHelloV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandAuthHelloV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandAuthHelloV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandAuthHelloV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_AUTH_HELLO_V1_PARAMS_TOKEN_PATTERN: Pattern = Pattern::new(r"^[A-Za-z0-9_-]{43}$");

/// Token da execução: 32 bytes aleatórios em base64url sem preenchimento (43 caracteres).
///
/// `params.token` in `urn:ads:command:auth.hello:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandAuthHelloV1ParamsToken(String);

impl CommandAuthHelloV1ParamsToken {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommandAuthHelloV1ParamsToken", text))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMAND_AUTH_HELLO_V1_PARAMS_TOKEN_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandAuthHelloV1ParamsToken {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandAuthHelloV1ParamsToken> for String {
    fn from(value: CommandAuthHelloV1ParamsToken) -> String {
        value.0
    }
}

impl fmt::Display for CommandAuthHelloV1ParamsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandAuthHelloV1ParamsToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
