// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.response.v2.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Rule};

static COMMAND_RESPONSE_V2_RULES: &[(&str, Rule)] = &[(
    "#/oneOf",
    Rule::OneOf(&[Rule::Required(&["result"]), Rule::Required(&["error"])]),
)];

/// Resposta de comando. Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois. A versão 2 fecha a forma do result: o comando foi aceito, e replayed diz se esta é a repetição de um comando já aceito com a mesma chave de idempotência (nada foi executado de novo). O efeito do comando volta como evento, não no result.
///
/// `urn:ads:command:response:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "CommandResponseV2Fields")]
pub struct CommandResponseV2(CommandResponseV2Fields);

/// The fields of [`CommandResponseV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResponseV2Fields {
    pub jsonrpc: CommandResponseV2Jsonrpc,
    pub id: CommonUlidV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub result: Option<CommandResponseV2Result>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub error: Option<CommandResponseV2Error>,
}

impl CommandResponseV2 {
    pub fn new(fields: CommandResponseV2Fields) -> Result<Self, ContractError> {
        support::check_rules("CommandResponseV2", &fields, COMMAND_RESPONSE_V2_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandResponseV2Fields {
        self.0
    }
}

impl Deref for CommandResponseV2 {
    type Target = CommandResponseV2Fields;

    fn deref(&self) -> &CommandResponseV2Fields {
        &self.0
    }
}

impl TryFrom<CommandResponseV2Fields> for CommandResponseV2 {
    type Error = ContractError;

    fn try_from(fields: CommandResponseV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandResponseV2> for CommandResponseV2Fields {
    fn from(value: CommandResponseV2) -> CommandResponseV2Fields {
        value.0
    }
}

impl Serialize for CommandResponseV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `jsonrpc` in `urn:ads:command:response:2`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandResponseV2Jsonrpc;

impl CommandResponseV2Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandResponseV2Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandResponseV2Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResponseV2Result {
    pub accepted: CommandResponseV2ResultAccepted,
    pub replayed: bool,
}

/// `result.accepted` in `urn:ads:command:response:2`.
///
/// Always `true`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandResponseV2ResultAccepted;

impl CommandResponseV2ResultAccepted {
    pub const VALUE: bool = true;
}

impl Serialize for CommandResponseV2ResultAccepted {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandResponseV2ResultAccepted {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_bool(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `error` in `urn:ads:command:response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResponseV2Error {
    pub code: i64,
    pub message: CommandResponseV2ErrorMessage,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub data: Option<Map<String, Value>>,
}

/// `error.message` in `urn:ads:command:response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandResponseV2ErrorMessage(String);

impl CommandResponseV2ErrorMessage {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommandResponseV2ErrorMessage", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommandResponseV2ErrorMessage {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandResponseV2ErrorMessage> for String {
    fn from(value: CommandResponseV2ErrorMessage) -> String {
        value.0
    }
}

impl fmt::Display for CommandResponseV2ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandResponseV2ErrorMessage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
