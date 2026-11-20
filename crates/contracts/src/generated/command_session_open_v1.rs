// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.session.open.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Abrir sessão. Pede ao núcleo uma sessão de terminal com uma ferramenta configurada, numa pasta configurada. A interface escolhe só pelo nome: a ferramenta pelo identificador do adaptador e a pasta pelo apelido. Programa, argumentos, ambiente e pasta vêm da configuração do usuário no núcleo, nunca do comando. O efeito volta como evento session.created.
///
/// `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionOpenV1 {
    pub jsonrpc: CommandSessionOpenV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandSessionOpenV1Method,
    pub params: CommandSessionOpenV1Params,
}

/// `jsonrpc` in `urn:ads:command:session.open:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionOpenV1Jsonrpc;

impl CommandSessionOpenV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandSessionOpenV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionOpenV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:session.open:1`.
///
/// Always `"session.open"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionOpenV1Method;

impl CommandSessionOpenV1Method {
    pub const VALUE: &str = "session.open";
}

impl Serialize for CommandSessionOpenV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionOpenV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionOpenV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandSessionOpenV1ParamsMeta,
    pub session_id: CommonIdV1,
    pub harness: CommonIdV1,
    pub workdir_alias: CommonIdV1,
    pub cols: CommandSessionOpenV1ParamsCols,
    pub rows: CommandSessionOpenV1ParamsRows,
}

static COMMAND_SESSION_OPEN_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:session.open:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandSessionOpenV1ParamsMetaFields")]
pub struct CommandSessionOpenV1ParamsMeta(CommandSessionOpenV1ParamsMetaFields);

/// The fields of [`CommandSessionOpenV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionOpenV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandSessionOpenV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandSessionOpenV1ParamsMetaRemote>,
}

impl CommandSessionOpenV1ParamsMeta {
    pub fn new(fields: CommandSessionOpenV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandSessionOpenV1ParamsMeta",
            &fields,
            COMMAND_SESSION_OPEN_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandSessionOpenV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandSessionOpenV1ParamsMeta {
    type Target = CommandSessionOpenV1ParamsMetaFields;

    fn deref(&self) -> &CommandSessionOpenV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandSessionOpenV1ParamsMetaFields> for CommandSessionOpenV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandSessionOpenV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandSessionOpenV1ParamsMeta> for CommandSessionOpenV1ParamsMetaFields {
    fn from(value: CommandSessionOpenV1ParamsMeta) -> CommandSessionOpenV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandSessionOpenV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandSessionOpenV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandSessionOpenV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionOpenV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandSessionOpenV1ParamsMetaRemoteSignature,
    pub nonce: CommandSessionOpenV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_SESSION_OPEN_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionOpenV1ParamsMetaRemoteSignature(String);

impl CommandSessionOpenV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionOpenV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_SESSION_OPEN_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionOpenV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionOpenV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandSessionOpenV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionOpenV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionOpenV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_SESSION_OPEN_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionOpenV1ParamsMetaRemoteNonce(String);

impl CommandSessionOpenV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionOpenV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_SESSION_OPEN_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionOpenV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionOpenV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandSessionOpenV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionOpenV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionOpenV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `params.cols` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct CommandSessionOpenV1ParamsCols(u16);

impl CommandSessionOpenV1ParamsCols {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "CommandSessionOpenV1ParamsCols",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for CommandSessionOpenV1ParamsCols {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<CommandSessionOpenV1ParamsCols> for u16 {
    fn from(value: CommandSessionOpenV1ParamsCols) -> u16 {
        value.0
    }
}

impl Serialize for CommandSessionOpenV1ParamsCols {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params.rows` in `urn:ads:command:session.open:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct CommandSessionOpenV1ParamsRows(u16);

impl CommandSessionOpenV1ParamsRows {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "CommandSessionOpenV1ParamsRows",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for CommandSessionOpenV1ParamsRows {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<CommandSessionOpenV1ParamsRows> for u16 {
    fn from(value: CommandSessionOpenV1ParamsRows) -> u16 {
        value.0
    }
}

impl Serialize for CommandSessionOpenV1ParamsRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
