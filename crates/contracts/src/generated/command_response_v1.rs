// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.response.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Rule};

static COMMAND_RESPONSE_V1_RULES: &[(&str, Rule)] = &[(
    "#/oneOf",
    Rule::OneOf(&[Rule::Required(&["result"]), Rule::Required(&["error"])]),
)];

/// Resposta de comando. Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois.
///
/// `urn:ads:command:response:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "CommandResponseV1Fields")]
pub struct CommandResponseV1(CommandResponseV1Fields);

/// The fields of [`CommandResponseV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResponseV1Fields {
    pub jsonrpc: CommandResponseV1Jsonrpc,
    pub id: CommonUlidV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub result: Option<Map<String, Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub error: Option<CommandResponseV1Error>,
}

impl CommandResponseV1 {
    pub fn new(fields: CommandResponseV1Fields) -> Result<Self, ContractError> {
        support::check_rules("CommandResponseV1", &fields, COMMAND_RESPONSE_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> CommandResponseV1Fields {
        self.0
    }
}

impl Deref for CommandResponseV1 {
    type Target = CommandResponseV1Fields;

    fn deref(&self) -> &CommandResponseV1Fields {
        &self.0
    }
}

impl TryFrom<CommandResponseV1Fields> for CommandResponseV1 {
    type Error = ContractError;

    fn try_from(fields: CommandResponseV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<CommandResponseV1> for CommandResponseV1Fields {
    fn from(value: CommandResponseV1) -> CommandResponseV1Fields {
        value.0
    }
}

impl Serialize for CommandResponseV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `jsonrpc` in `urn:ads:command:response:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandResponseV1Jsonrpc;

impl CommandResponseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandResponseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandResponseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `error` in `urn:ads:command:response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResponseV1Error {
    pub code: i64,
    pub message: CommandResponseV1ErrorMessage,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub data: Option<Map<String, Value>>,
}

/// `error.message` in `urn:ads:command:response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommandResponseV1ErrorMessage(String);

impl CommandResponseV1ErrorMessage {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommandResponseV1ErrorMessage", text))
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

impl TryFrom<String> for CommandResponseV1ErrorMessage {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommandResponseV1ErrorMessage> for String {
    fn from(value: CommandResponseV1ErrorMessage) -> String {
        value.0
    }
}

impl fmt::Display for CommandResponseV1ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommandResponseV1ErrorMessage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
