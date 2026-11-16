// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.session.resize.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Redimensionar sessão. Novo tamanho do terminal da sessão, em colunas e linhas, de 1 a 1000 como no contrato pty-host/1. O efeito volta como evento session.resized.
///
/// `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionResizeV1 {
    pub jsonrpc: CommandSessionResizeV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandSessionResizeV1Method,
    pub params: CommandSessionResizeV1Params,
}

/// `jsonrpc` in `urn:ads:command:session.resize:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionResizeV1Jsonrpc;

impl CommandSessionResizeV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandSessionResizeV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionResizeV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:session.resize:1`.
///
/// Always `"session.resize"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionResizeV1Method;

impl CommandSessionResizeV1Method {
    pub const VALUE: &str = "session.resize";
}

impl Serialize for CommandSessionResizeV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionResizeV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionResizeV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandSessionResizeV1ParamsMeta,
    pub session_id: CommonIdV1,
    pub cols: CommandSessionResizeV1ParamsCols,
    pub rows: CommandSessionResizeV1ParamsRows,
}

static COMMAND_SESSION_RESIZE_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:session.resize:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandSessionResizeV1ParamsMetaFields")]
pub struct CommandSessionResizeV1ParamsMeta(CommandSessionResizeV1ParamsMetaFields);

/// The fields of [`CommandSessionResizeV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionResizeV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandSessionResizeV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandSessionResizeV1ParamsMetaRemote>,
}

impl CommandSessionResizeV1ParamsMeta {
    pub fn new(fields: CommandSessionResizeV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandSessionResizeV1ParamsMeta",
            &fields,
            COMMAND_SESSION_RESIZE_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandSessionResizeV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandSessionResizeV1ParamsMeta {
    type Target = CommandSessionResizeV1ParamsMetaFields;

    fn deref(&self) -> &CommandSessionResizeV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandSessionResizeV1ParamsMetaFields> for CommandSessionResizeV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandSessionResizeV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandSessionResizeV1ParamsMeta> for CommandSessionResizeV1ParamsMetaFields {
    fn from(value: CommandSessionResizeV1ParamsMeta) -> CommandSessionResizeV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandSessionResizeV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandSessionResizeV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandSessionResizeV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionResizeV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandSessionResizeV1ParamsMetaRemoteSignature,
    pub nonce: CommandSessionResizeV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_SESSION_RESIZE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionResizeV1ParamsMetaRemoteSignature(String);

impl CommandSessionResizeV1ParamsMetaRemoteSignature {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionResizeV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == 88 && COMMAND_SESSION_RESIZE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionResizeV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionResizeV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandSessionResizeV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionResizeV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionResizeV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_SESSION_RESIZE_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionResizeV1ParamsMetaRemoteNonce(String);

impl CommandSessionResizeV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionResizeV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_SESSION_RESIZE_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionResizeV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionResizeV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandSessionResizeV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionResizeV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionResizeV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `params.cols` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct CommandSessionResizeV1ParamsCols(u16);

impl CommandSessionResizeV1ParamsCols {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "CommandSessionResizeV1ParamsCols",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for CommandSessionResizeV1ParamsCols {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<CommandSessionResizeV1ParamsCols> for u16 {
    fn from(value: CommandSessionResizeV1ParamsCols) -> u16 {
        value.0
    }
}

impl Serialize for CommandSessionResizeV1ParamsCols {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params.rows` in `urn:ads:command:session.resize:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct CommandSessionResizeV1ParamsRows(u16);

impl CommandSessionResizeV1ParamsRows {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "CommandSessionResizeV1ParamsRows",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for CommandSessionResizeV1ParamsRows {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<CommandSessionResizeV1ParamsRows> for u16 {
    fn from(value: CommandSessionResizeV1ParamsRows) -> u16 {
        value.0
    }
}

impl Serialize for CommandSessionResizeV1ParamsRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
