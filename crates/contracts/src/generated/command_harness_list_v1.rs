// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.harness.list.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

/// Listar ferramentas. Pede ao núcleo as ferramentas que a configuração do usuário liga e os apelidos de pasta de trabalho conhecidos. Só leitura: não muda nada no núcleo. A resposta aceita é command.harness.list.response:1.
///
/// `urn:ads:command:harness.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListV1 {
    pub jsonrpc: CommandHarnessListV1Jsonrpc,
    pub id: CommonUlidV1,
    pub method: CommandHarnessListV1Method,
    pub params: CommandHarnessListV1Params,
}

/// `jsonrpc` in `urn:ads:command:harness.list:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandHarnessListV1Jsonrpc;

impl CommandHarnessListV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandHarnessListV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandHarnessListV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `method` in `urn:ads:command:harness.list:1`.
///
/// Always `"harness.list"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandHarnessListV1Method;

impl CommandHarnessListV1Method {
    pub const VALUE: &str = "harness.list";
}

impl Serialize for CommandHarnessListV1Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandHarnessListV1Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `params` in `urn:ads:command:harness.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListV1Params {
    #[serde(rename = "_meta")]
    pub meta: CommandHarnessListV1ParamsMeta,
}

static COMMAND_HARNESS_LIST_V1_PARAMS_META_RULES: &[(&str, Rule)] = &[(
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

/// `params._meta` in `urn:ads:command:harness.list:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "CommandHarnessListV1ParamsMetaFields")]
pub struct CommandHarnessListV1ParamsMeta(CommandHarnessListV1ParamsMetaFields);

/// The fields of [`CommandHarnessListV1ParamsMeta`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListV1ParamsMetaFields {
    pub idempotency_key: CommonUlidV1,
    pub origin: CommandHarnessListV1ParamsMetaOrigin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub remote: Option<CommandHarnessListV1ParamsMetaRemote>,
}

impl CommandHarnessListV1ParamsMeta {
    pub fn new(fields: CommandHarnessListV1ParamsMetaFields) -> Result<Self, ContractError> {
        support::check_rules(
            "CommandHarnessListV1ParamsMeta",
            &fields,
            COMMAND_HARNESS_LIST_V1_PARAMS_META_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandHarnessListV1ParamsMetaFields {
        self.0
    }
}

impl Deref for CommandHarnessListV1ParamsMeta {
    type Target = CommandHarnessListV1ParamsMetaFields;

    fn deref(&self) -> &CommandHarnessListV1ParamsMetaFields {
        &self.0
    }
}

impl TryFrom<CommandHarnessListV1ParamsMetaFields> for CommandHarnessListV1ParamsMeta {
    type Error = ContractError;

    fn try_from(fields: CommandHarnessListV1ParamsMetaFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandHarnessListV1ParamsMeta> for CommandHarnessListV1ParamsMetaFields {
    fn from(value: CommandHarnessListV1ParamsMeta) -> CommandHarnessListV1ParamsMetaFields {
        value.0
    }
}

impl Serialize for CommandHarnessListV1ParamsMeta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `params._meta.origin` in `urn:ads:command:harness.list:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandHarnessListV1ParamsMetaOrigin {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl CommandHarnessListV1ParamsMetaOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// `params._meta.remote` in `urn:ads:command:harness.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListV1ParamsMetaRemote {
    pub device_id: CommonIdV1,
    pub signature: CommandHarnessListV1ParamsMetaRemoteSignature,
    pub nonce: CommandHarnessListV1ParamsMetaRemoteNonce,
    pub counter: NonZeroU64,
    pub expires_at: CommonTimestampV1,
}

static COMMAND_HARNESS_LIST_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.signature` in `urn:ads:command:harness.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandHarnessListV1ParamsMetaRemoteSignature(String);

impl CommandHarnessListV1ParamsMetaRemoteSignature {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandHarnessListV1ParamsMetaRemoteSignature",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length == 88 && COMMAND_HARNESS_LIST_V1_PARAMS_META_REMOTE_SIGNATURE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandHarnessListV1ParamsMetaRemoteSignature {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandHarnessListV1ParamsMetaRemoteSignature> for String {
    fn from(value: CommandHarnessListV1ParamsMetaRemoteSignature) -> String {
        value.0
    }
}

impl fmt::Display for CommandHarnessListV1ParamsMetaRemoteSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandHarnessListV1ParamsMetaRemoteSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static COMMAND_HARNESS_LIST_V1_PARAMS_META_REMOTE_NONCE_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `params._meta.remote.nonce` in `urn:ads:command:harness.list:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandHarnessListV1ParamsMetaRemoteNonce(String);

impl CommandHarnessListV1ParamsMetaRemoteNonce {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "CommandHarnessListV1ParamsMetaRemoteNonce",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 22 && COMMAND_HARNESS_LIST_V1_PARAMS_META_REMOTE_NONCE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandHarnessListV1ParamsMetaRemoteNonce {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandHarnessListV1ParamsMetaRemoteNonce> for String {
    fn from(value: CommandHarnessListV1ParamsMetaRemoteNonce) -> String {
        value.0
    }
}

impl fmt::Display for CommandHarnessListV1ParamsMetaRemoteNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandHarnessListV1ParamsMetaRemoteNonce {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
