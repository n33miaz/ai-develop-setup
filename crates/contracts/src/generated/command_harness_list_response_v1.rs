// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.harness.list.response.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::ContractError;

/// Resposta de harness.list. Resposta JSON-RPC 2.0 aceita de harness.list: os identificadores das ferramentas ligadas na configuração do usuário e os apelidos de pasta de trabalho, só nomes. Nunca leva programa, argumentos, ambiente nem caminho. A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:harness.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListResponseV1 {
    pub jsonrpc: CommandHarnessListResponseV1Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandHarnessListResponseV1Result,
}

/// `jsonrpc` in `urn:ads:command:harness.list.response:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandHarnessListResponseV1Jsonrpc;

impl CommandHarnessListResponseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandHarnessListResponseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandHarnessListResponseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:harness.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListResponseV1Result {
    pub harnesses: CommandHarnessListResponseV1ResultHarnesses,
    pub workdir_aliases: CommandHarnessListResponseV1ResultWorkdirAliases,
}

/// `result.harnesses` in `urn:ads:command:harness.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct CommandHarnessListResponseV1ResultHarnesses(Vec<CommonIdV1>);

impl CommandHarnessListResponseV1ResultHarnesses {
    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandHarnessListResponseV1ResultHarnesses",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommonIdV1] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommonIdV1> {
        self.0
    }
}

impl TryFrom<Vec<CommonIdV1>> for CommandHarnessListResponseV1ResultHarnesses {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandHarnessListResponseV1ResultHarnesses> for Vec<CommonIdV1> {
    fn from(value: CommandHarnessListResponseV1ResultHarnesses) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for CommandHarnessListResponseV1ResultHarnesses {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `result.workdir_aliases` in `urn:ads:command:harness.list.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct CommandHarnessListResponseV1ResultWorkdirAliases(Vec<CommonIdV1>);

impl CommandHarnessListResponseV1ResultWorkdirAliases {
    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandHarnessListResponseV1ResultWorkdirAliases",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommonIdV1] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommonIdV1> {
        self.0
    }
}

impl TryFrom<Vec<CommonIdV1>> for CommandHarnessListResponseV1ResultWorkdirAliases {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandHarnessListResponseV1ResultWorkdirAliases> for Vec<CommonIdV1> {
    fn from(value: CommandHarnessListResponseV1ResultWorkdirAliases) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for CommandHarnessListResponseV1ResultWorkdirAliases {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
