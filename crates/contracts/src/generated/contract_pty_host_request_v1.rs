// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.pty-host.request.v1.schema.json. Não edite à mão.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_base64_v1::CommonBase64V1;
use super::common_id_v1::CommonIdV1;
use crate::support;
use crate::support::ContractError;

/// Pedido ao host de PTY. Pedido JSON-RPC 2.0 do núcleo ao host de PTY no contrato pty-host/1, com envelope próprio: id inteiro por conexão, sem chave de idempotência nem assinatura, porque o canal é local e do próprio usuário. Os bytes de saída do terminal não vêm em resposta: viajam no quadro binário.
///
/// `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum ContractPtyHostRequestV1 {
    #[serde(rename = "host.hello")]
    HostHello(ContractPtyHostRequestV1HostHello),
    #[serde(rename = "session.open")]
    SessionOpen(ContractPtyHostRequestV1SessionOpen),
    #[serde(rename = "session.write")]
    SessionWrite(ContractPtyHostRequestV1SessionWrite),
    #[serde(rename = "session.resize")]
    SessionResize(ContractPtyHostRequestV1SessionResize),
    #[serde(rename = "session.subscribe")]
    SessionSubscribe(ContractPtyHostRequestV1SessionSubscribe),
    #[serde(rename = "session.close")]
    SessionClose(ContractPtyHostRequestV1SessionClose),
    #[serde(rename = "session.list")]
    SessionList(ContractPtyHostRequestV1SessionList),
}

/// `host.hello` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1HostHello {
    pub jsonrpc: ContractPtyHostRequestV1HostHelloJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1HostHelloParams,
}

/// `host.hello.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1HostHelloJsonrpc;

impl ContractPtyHostRequestV1HostHelloJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1HostHelloJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1HostHelloJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `host.hello.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1HostHelloParams {
    pub contract: ContractPtyHostRequestV1HostHelloParamsContract,
}

/// `host.hello.params.contract` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"pty-host/1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1HostHelloParamsContract;

impl ContractPtyHostRequestV1HostHelloParamsContract {
    pub const VALUE: &str = "pty-host/1";
}

impl Serialize for ContractPtyHostRequestV1HostHelloParamsContract {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1HostHelloParamsContract {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.open` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionOpen {
    pub jsonrpc: ContractPtyHostRequestV1SessionOpenJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1SessionOpenParams,
}

/// `session.open.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1SessionOpenJsonrpc;

impl ContractPtyHostRequestV1SessionOpenJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1SessionOpenJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1SessionOpenJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.open.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionOpenParams {
    pub session_id: CommonIdV1,
    pub program: ContractPtyHostRequestV1SessionOpenParamsProgram,
    pub args: Vec<ContractPtyHostRequestV1SessionOpenParamsArgsItem>,
    #[serde(deserialize_with = "support::nullable")]
    pub cwd: Option<ContractPtyHostRequestV1SessionOpenParamsCwd>,
    pub env: BTreeMap<String, String>,
    pub cols: ContractPtyHostRequestV1SessionOpenParamsCols,
    pub rows: ContractPtyHostRequestV1SessionOpenParamsRows,
}

/// `session.open.params.program` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct ContractPtyHostRequestV1SessionOpenParamsProgram(String);

impl ContractPtyHostRequestV1SessionOpenParamsProgram {
    pub const MAX_CHARS: usize = 1024;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionOpenParamsProgram",
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

impl TryFrom<String> for ContractPtyHostRequestV1SessionOpenParamsProgram {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<ContractPtyHostRequestV1SessionOpenParamsProgram> for String {
    fn from(value: ContractPtyHostRequestV1SessionOpenParamsProgram) -> String {
        value.0
    }
}

impl fmt::Display for ContractPtyHostRequestV1SessionOpenParamsProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ContractPtyHostRequestV1SessionOpenParamsProgram {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `session.open.params.args[]` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct ContractPtyHostRequestV1SessionOpenParamsArgsItem(String);

impl ContractPtyHostRequestV1SessionOpenParamsArgsItem {
    pub const MAX_CHARS: usize = 8192;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionOpenParamsArgsItem",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= Self::MAX_CHARS
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContractPtyHostRequestV1SessionOpenParamsArgsItem {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<ContractPtyHostRequestV1SessionOpenParamsArgsItem> for String {
    fn from(value: ContractPtyHostRequestV1SessionOpenParamsArgsItem) -> String {
        value.0
    }
}

impl fmt::Display for ContractPtyHostRequestV1SessionOpenParamsArgsItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ContractPtyHostRequestV1SessionOpenParamsArgsItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `session.open.params.cwd` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct ContractPtyHostRequestV1SessionOpenParamsCwd(String);

impl ContractPtyHostRequestV1SessionOpenParamsCwd {
    pub const MAX_CHARS: usize = 1024;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionOpenParamsCwd",
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

impl TryFrom<String> for ContractPtyHostRequestV1SessionOpenParamsCwd {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<ContractPtyHostRequestV1SessionOpenParamsCwd> for String {
    fn from(value: ContractPtyHostRequestV1SessionOpenParamsCwd) -> String {
        value.0
    }
}

impl fmt::Display for ContractPtyHostRequestV1SessionOpenParamsCwd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ContractPtyHostRequestV1SessionOpenParamsCwd {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `session.open.params.cols` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct ContractPtyHostRequestV1SessionOpenParamsCols(u16);

impl ContractPtyHostRequestV1SessionOpenParamsCols {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionOpenParamsCols",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ContractPtyHostRequestV1SessionOpenParamsCols {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<ContractPtyHostRequestV1SessionOpenParamsCols> for u16 {
    fn from(value: ContractPtyHostRequestV1SessionOpenParamsCols) -> u16 {
        value.0
    }
}

impl Serialize for ContractPtyHostRequestV1SessionOpenParamsCols {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `session.open.params.rows` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct ContractPtyHostRequestV1SessionOpenParamsRows(u16);

impl ContractPtyHostRequestV1SessionOpenParamsRows {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionOpenParamsRows",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ContractPtyHostRequestV1SessionOpenParamsRows {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<ContractPtyHostRequestV1SessionOpenParamsRows> for u16 {
    fn from(value: ContractPtyHostRequestV1SessionOpenParamsRows) -> u16 {
        value.0
    }
}

impl Serialize for ContractPtyHostRequestV1SessionOpenParamsRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `session.write` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionWrite {
    pub jsonrpc: ContractPtyHostRequestV1SessionWriteJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1SessionWriteParams,
}

/// `session.write.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1SessionWriteJsonrpc;

impl ContractPtyHostRequestV1SessionWriteJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1SessionWriteJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1SessionWriteJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.write.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionWriteParams {
    pub session_id: CommonIdV1,
    pub data: CommonBase64V1,
}

/// `session.resize` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionResize {
    pub jsonrpc: ContractPtyHostRequestV1SessionResizeJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1SessionResizeParams,
}

/// `session.resize.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1SessionResizeJsonrpc;

impl ContractPtyHostRequestV1SessionResizeJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1SessionResizeJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1SessionResizeJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.resize.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionResizeParams {
    pub session_id: CommonIdV1,
    pub cols: ContractPtyHostRequestV1SessionResizeParamsCols,
    pub rows: ContractPtyHostRequestV1SessionResizeParamsRows,
}

/// `session.resize.params.cols` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct ContractPtyHostRequestV1SessionResizeParamsCols(u16);

impl ContractPtyHostRequestV1SessionResizeParamsCols {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionResizeParamsCols",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ContractPtyHostRequestV1SessionResizeParamsCols {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<ContractPtyHostRequestV1SessionResizeParamsCols> for u16 {
    fn from(value: ContractPtyHostRequestV1SessionResizeParamsCols) -> u16 {
        value.0
    }
}

impl Serialize for ContractPtyHostRequestV1SessionResizeParamsCols {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `session.resize.params.rows` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct ContractPtyHostRequestV1SessionResizeParamsRows(u16);

impl ContractPtyHostRequestV1SessionResizeParamsRows {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "ContractPtyHostRequestV1SessionResizeParamsRows",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ContractPtyHostRequestV1SessionResizeParamsRows {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<ContractPtyHostRequestV1SessionResizeParamsRows> for u16 {
    fn from(value: ContractPtyHostRequestV1SessionResizeParamsRows) -> u16 {
        value.0
    }
}

impl Serialize for ContractPtyHostRequestV1SessionResizeParamsRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `session.subscribe` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionSubscribe {
    pub jsonrpc: ContractPtyHostRequestV1SessionSubscribeJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1SessionSubscribeParams,
}

/// `session.subscribe.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1SessionSubscribeJsonrpc;

impl ContractPtyHostRequestV1SessionSubscribeJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1SessionSubscribeJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1SessionSubscribeJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.subscribe.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionSubscribeParams {
    pub session_id: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub from_seq: Option<NonZeroU64>,
}

/// `session.close` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionClose {
    pub jsonrpc: ContractPtyHostRequestV1SessionCloseJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1SessionCloseParams,
}

/// `session.close.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1SessionCloseJsonrpc;

impl ContractPtyHostRequestV1SessionCloseJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1SessionCloseJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1SessionCloseJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.close.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionCloseParams {
    pub session_id: CommonIdV1,
}

/// `session.list` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionList {
    pub jsonrpc: ContractPtyHostRequestV1SessionListJsonrpc,
    pub id: NonZeroU64,
    pub params: ContractPtyHostRequestV1SessionListParams,
}

/// `session.list.jsonrpc` in `urn:ads:contract:pty-host.request:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostRequestV1SessionListJsonrpc;

impl ContractPtyHostRequestV1SessionListJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostRequestV1SessionListJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostRequestV1SessionListJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.list.params` in `urn:ads:contract:pty-host.request:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostRequestV1SessionListParams {}
