// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.session.write.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Escrever na sessão. Bytes digitados ou colados pela interface, em base64, para a entrada do terminal da sessão. De 1 a 65536 bytes por comando (87384 caracteres em base64): cabe com folga no quadro padrão do contrato pty-host/1; texto maior vai em vários comandos, na ordem.
///
/// `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionWriteV1 {
    pub jsonrpc: CommandSessionWriteV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandSessionWriteV1Method,
    pub params: CommandSessionWriteV1Params,
}

/// `jsonrpc` in `urn:ads:command:session.write:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionWriteV1Jsonrpc;

impl CommandSessionWriteV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandSessionWriteV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionWriteV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:session.write:1`.
///
/// Always `"session.write"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandSessionWriteV1Method;

impl CommandSessionWriteV1Method {
    pub const VALUE: &str = "session.write";
}

impl Serialize for CommandSessionWriteV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandSessionWriteV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionWriteV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandSessionWriteV1ParamsMeta,
    pub session_id: CommonIdV1,
    pub data: CommandSessionWriteV1ParamsData,
}

static COMMAND_SESSION_WRITE_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:session.write:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandSessionWriteV1ParamsMetaFields")]
pub struct CommandSessionWriteV1ParamsMeta(CommandSessionWriteV1ParamsMetaFields);

/// The fields of [`CommandSessionWriteV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionWriteV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandSessionWriteV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandSessionWriteV1ParamsMetaRemote>,
}

impl CommandSessionWriteV1ParamsMeta {
    pub fn new(fields: CommandSessionWriteV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandSessionWriteV1ParamsMeta",
            &fields,
            COMMAND_SESSION_WRITE_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandSessionWriteV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandSessionWriteV1ParamsMeta {
    type Target = CommandSessionWriteV1ParamsMetaFields;

    fn deref(&self) -> &CommandSessionWriteV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandSessionWriteV1ParamsMetaFields> for CommandSessionWriteV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandSessionWriteV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandSessionWriteV1ParamsMeta> for CommandSessionWriteV1ParamsMetaFields {
    fn from(value: CommandSessionWriteV1ParamsMeta) -> CommandSessionWriteV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandSessionWriteV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandSessionWriteV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandSessionWriteV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSessionWriteV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandSessionWriteV1ParamsMetaRemoteSignature,
    pub nonce: CommandSessionWriteV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_SESSION_WRITE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionWriteV1ParamsMetaRemoteSignature(String);

impl CommandSessionWriteV1ParamsMetaRemoteSignature {
    pub const MAX_CHARS: usize = 88;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionWriteV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == Self::MAX_CHARS
            && COMMAND_SESSION_WRITE_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionWriteV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionWriteV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandSessionWriteV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionWriteV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionWriteV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_SESSION_WRITE_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionWriteV1ParamsMetaRemoteNonce(String);

impl CommandSessionWriteV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandSessionWriteV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_SESSION_WRITE_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionWriteV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionWriteV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandSessionWriteV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionWriteV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionWriteV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_SESSION_WRITE_V1_PARAMS_DATA_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// Bytes em base64 padrão (RFC 4648) com preenchimento: de 1 a 65536 bytes.
///
/// `params.data` in `urn:ads:command:session.write:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandSessionWriteV1ParamsData(String);

impl CommandSessionWriteV1ParamsData {
    pub const MAX_CHARS: usize = 87384;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommandSessionWriteV1ParamsData", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (4..=Self::MAX_CHARS).contains(&length)
            && COMMAND_SESSION_WRITE_V1_PARAMS_DATA_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandSessionWriteV1ParamsData {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandSessionWriteV1ParamsData> for String {
    fn from(value: CommandSessionWriteV1ParamsData) -> String {
        value.0
    }
}

impl fmt::Display for CommandSessionWriteV1ParamsData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandSessionWriteV1ParamsData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
