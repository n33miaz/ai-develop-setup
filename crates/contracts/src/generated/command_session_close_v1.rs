// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.session.close.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Fechar sessão. Pede ao núcleo que encerre a sessão de terminal e o processo dela. O efeito volta como evento session.ended.
///
/// `urn:ads:command:session.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionCloseV1 {
    pub jsonrpc: CommandSessionCloseV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandSessionCloseV1Method,
    pub params: CommandSessionCloseV1Params,
}

/// `jsonrpc` in `urn:ads:command:session.close:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionCloseV1Jsonrpc;

impl CommandSessionCloseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandSessionCloseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionCloseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:session.close:1`.
///
/// Always `"session.close"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionCloseV1Method;

impl CommandSessionCloseV1Method {
    pub const VALUE: &str = "session.close";
}

impl Serialize for CommandSessionCloseV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionCloseV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:session.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionCloseV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandSessionCloseV1ParamsMeta,
    pub session_id: CommonIdV1,
}

static COMMAND_SESSION_CLOSE_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:session.close:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandSessionCloseV1ParamsMetaFields")]
pub struct CommandSessionCloseV1ParamsMeta(CommandSessionCloseV1ParamsMetaFields);

/// The fields of [`CommandSessionCloseV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionCloseV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandSessionCloseV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandSessionCloseV1ParamsMetaRemote>,
}

impl CommandSessionCloseV1ParamsMeta {
    pub fn new(fields: CommandSessionCloseV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandSessionCloseV1ParamsMeta",
            &fields,
            COMMAND_SESSION_CLOSE_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandSessionCloseV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandSessionCloseV1ParamsMeta {
    type Target = CommandSessionCloseV1ParamsMetaFields;

    fn deref(&self) -> &CommandSessionCloseV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandSessionCloseV1ParamsMetaFields> for CommandSessionCloseV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandSessionCloseV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandSessionCloseV1ParamsMeta> for CommandSessionCloseV1ParamsMetaFields {
    fn from(value: CommandSessionCloseV1ParamsMeta) -> CommandSessionCloseV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandSessionCloseV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:session.close:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandSessionCloseV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandSessionCloseV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:session.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionCloseV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandSessionCloseV1ParamsMetaRemoteSignature,
    pub nonce: CommandSessionCloseV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_SESSION_CLOSE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:session.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionCloseV1ParamsMetaRemoteSignature(String);

impl CommandSessionCloseV1ParamsMetaRemoteSignature {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionCloseV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == 88 && COMMAND_SESSION_CLOSE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionCloseV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionCloseV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandSessionCloseV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionCloseV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionCloseV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_SESSION_CLOSE_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:session.close:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionCloseV1ParamsMetaRemoteNonce(String);

impl CommandSessionCloseV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionCloseV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_SESSION_CLOSE_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionCloseV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionCloseV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandSessionCloseV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionCloseV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionCloseV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
