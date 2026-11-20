// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.pty-host.response.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Rule};

static CONTRACT_PTY_HOST_RESPONSE_V1_RULES: &[(&str, Rule)] = &[(
    "#/oneOf",
    Rule::OneOf(&[Rule::Required(&["result"]), Rule::Required(&["error"])]),
)];

/// Resposta do host de PTY. Resposta JSON-RPC 2.0 do host de PTY: ou result ou error, nunca os dois. O result tem uma das formas do contrato (vazio, saudação, limites do replay, encerramento ou lista de sessões). Erro de replay informa o limite: nada é pulado em silêncio.
///
/// `urn:ads:contract:pty-host.response:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "ContractPtyHostResponseV1Fields")]
pub struct ContractPtyHostResponseV1(ContractPtyHostResponseV1Fields);

/// The fields of [`ContractPtyHostResponseV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1Fields {
    pub jsonrpc: ContractPtyHostResponseV1Jsonrpc,
    #[serde(deserialize_with = "support::nullable")]
    pub id: Option<NonZeroU64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub result: Option<ContractPtyHostResponseV1Result>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub error: Option<ContractPtyHostResponseV1Error>,
}

impl ContractPtyHostResponseV1 {
    pub fn new(fields: ContractPtyHostResponseV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "ContractPtyHostResponseV1",
            &fields,
            CONTRACT_PTY_HOST_RESPONSE_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> ContractPtyHostResponseV1Fields {
        self.0
    }
}

impl Deref for ContractPtyHostResponseV1 {
    type Target = ContractPtyHostResponseV1Fields;

    fn deref(&self) -> &ContractPtyHostResponseV1Fields {
        &self.0
    }
}

impl TryFrom<ContractPtyHostResponseV1Fields> for ContractPtyHostResponseV1 {
    type Error = ContractError;

    fn try_from(fields: ContractPtyHostResponseV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<ContractPtyHostResponseV1> for ContractPtyHostResponseV1Fields {
    fn from(value: ContractPtyHostResponseV1) -> ContractPtyHostResponseV1Fields {
        value.0
    }
}

impl Serialize for ContractPtyHostResponseV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `jsonrpc` in `urn:ads:contract:pty-host.response:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostResponseV1Jsonrpc;

impl ContractPtyHostResponseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostResponseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostResponseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractPtyHostResponseV1Result {
    Empty(ContractPtyHostResponseV1Empty),
    Hello(ContractPtyHostResponseV1Hello),
    ReplayBounds(ContractPtyHostResponseV1ReplayBounds),
    Closed(ContractPtyHostResponseV1Closed),
    SessionList(ContractPtyHostResponseV1SessionList),
}

/// `$defs.empty` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1Empty {}

/// `$defs.hello` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1Hello {
    pub contract: ContractPtyHostResponseV1HelloContract,
}

/// `$defs.hello.contract` in `urn:ads:contract:pty-host.response:1`.
///
/// Always `"pty-host/1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostResponseV1HelloContract;

impl ContractPtyHostResponseV1HelloContract {
    pub const VALUE: &str = "pty-host/1";
}

impl Serialize for ContractPtyHostResponseV1HelloContract {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostResponseV1HelloContract {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `$defs.replay_bounds` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1ReplayBounds {
    pub oldest_seq: NonZeroU64,
    pub next_seq: NonZeroU64,
}

/// `$defs.closed` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1Closed {
    #[serde(deserialize_with = "support::nullable")]
    pub exit_code: Option<i64>,
}

/// `$defs.session_list` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1SessionList {
    pub sessions: Vec<ContractPtyHostResponseV1SessionListSessionsItem>,
}

/// `$defs.session_list.sessions[]` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1SessionListSessionsItem {
    pub session_id: CommonIdV1,
    pub running: bool,
    #[serde(deserialize_with = "support::nullable")]
    pub exit_code: Option<i64>,
    pub oldest_seq: NonZeroU64,
    pub next_seq: NonZeroU64,
}

static CONTRACT_PTY_HOST_RESPONSE_V1_ERROR_RULES: &[(&str, Rule)] = &[
    (
        "#/properties/error/allOf/0",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("code", Rule::Const(Literal::Int(-32004)))]),
                Rule::Required(&["code"]),
            ]),
            then: Some(&Rule::AllOf(&[
                Rule::Required(&["data"]),
                Rule::Properties(&[(
                    "data",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Required(&["oldest_seq"]),
                    ]),
                )]),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/properties/error/allOf/1",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("code", Rule::Const(Literal::Int(-32005)))]),
                Rule::Required(&["code"]),
            ]),
            then: Some(&Rule::AllOf(&[
                Rule::Required(&["data"]),
                Rule::Properties(&[(
                    "data",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Required(&["next_seq"]),
                    ]),
                )]),
            ])),
            otherwise: None,
        },
    ),
];

/// `error` in `urn:ads:contract:pty-host.response:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "ContractPtyHostResponseV1ErrorFields")]
pub struct ContractPtyHostResponseV1Error(ContractPtyHostResponseV1ErrorFields);

/// The fields of [`ContractPtyHostResponseV1Error`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1ErrorFields {
    pub code: ContractPtyHostResponseV1ErrorCode,
    pub message: ContractPtyHostResponseV1ErrorMessage,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub data: Option<ContractPtyHostResponseV1ErrorData>,
}

impl ContractPtyHostResponseV1Error {
    pub fn new(fields: ContractPtyHostResponseV1ErrorFields) -> Result<Self, ContractError> {
        support::check_rules(
            "ContractPtyHostResponseV1Error",
            &fields,
            CONTRACT_PTY_HOST_RESPONSE_V1_ERROR_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> ContractPtyHostResponseV1ErrorFields {
        self.0
    }
}

impl Deref for ContractPtyHostResponseV1Error {
    type Target = ContractPtyHostResponseV1ErrorFields;

    fn deref(&self) -> &ContractPtyHostResponseV1ErrorFields {
        &self.0
    }
}

impl TryFrom<ContractPtyHostResponseV1ErrorFields> for ContractPtyHostResponseV1Error {
    type Error = ContractError;

    fn try_from(fields: ContractPtyHostResponseV1ErrorFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<ContractPtyHostResponseV1Error> for ContractPtyHostResponseV1ErrorFields {
    fn from(value: ContractPtyHostResponseV1Error) -> ContractPtyHostResponseV1ErrorFields {
        value.0
    }
}

impl Serialize for ContractPtyHostResponseV1Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `error.code` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub enum ContractPtyHostResponseV1ErrorCode {
    Neg32700,
    Neg32600,
    Neg32601,
    Neg32602,
    Neg32603,
    Neg32001,
    Neg32002,
    Neg32003,
    Neg32004,
    Neg32005,
    Neg32006,
    Neg32007,
    Neg32008,
    Neg32009,
}

impl ContractPtyHostResponseV1ErrorCode {
    pub fn number(self) -> i64 {
        match self {
            Self::Neg32700 => -32700,
            Self::Neg32600 => -32600,
            Self::Neg32601 => -32601,
            Self::Neg32602 => -32602,
            Self::Neg32603 => -32603,
            Self::Neg32001 => -32001,
            Self::Neg32002 => -32002,
            Self::Neg32003 => -32003,
            Self::Neg32004 => -32004,
            Self::Neg32005 => -32005,
            Self::Neg32006 => -32006,
            Self::Neg32007 => -32007,
            Self::Neg32008 => -32008,
            Self::Neg32009 => -32009,
        }
    }
}

impl TryFrom<i64> for ContractPtyHostResponseV1ErrorCode {
    type Error = ContractError;

    fn try_from(number: i64) -> Result<Self, ContractError> {
        match number {
            -32700 => Ok(Self::Neg32700),
            -32600 => Ok(Self::Neg32600),
            -32601 => Ok(Self::Neg32601),
            -32602 => Ok(Self::Neg32602),
            -32603 => Ok(Self::Neg32603),
            -32001 => Ok(Self::Neg32001),
            -32002 => Ok(Self::Neg32002),
            -32003 => Ok(Self::Neg32003),
            -32004 => Ok(Self::Neg32004),
            -32005 => Ok(Self::Neg32005),
            -32006 => Ok(Self::Neg32006),
            -32007 => Ok(Self::Neg32007),
            -32008 => Ok(Self::Neg32008),
            -32009 => Ok(Self::Neg32009),
            other => Err(ContractError::new(
                "ContractPtyHostResponseV1ErrorCode",
                other.to_string(),
            )),
        }
    }
}

impl From<ContractPtyHostResponseV1ErrorCode> for i64 {
    fn from(value: ContractPtyHostResponseV1ErrorCode) -> i64 {
        value.number()
    }
}

/// `error.message` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct ContractPtyHostResponseV1ErrorMessage(String);

impl ContractPtyHostResponseV1ErrorMessage {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "ContractPtyHostResponseV1ErrorMessage",
                text,
            ))
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

impl TryFrom<String> for ContractPtyHostResponseV1ErrorMessage {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<ContractPtyHostResponseV1ErrorMessage> for String {
    fn from(value: ContractPtyHostResponseV1ErrorMessage) -> String {
        value.0
    }
}

impl fmt::Display for ContractPtyHostResponseV1ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ContractPtyHostResponseV1ErrorMessage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `error.data` in `urn:ads:contract:pty-host.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostResponseV1ErrorData {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub oldest_seq: Option<NonZeroU64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub next_seq: Option<NonZeroU64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub limit: Option<NonZeroU64>,
}
