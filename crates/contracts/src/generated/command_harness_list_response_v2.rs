// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.harness.list.response.v2.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::ContractError;

/// Resposta de harness.list. Resposta JSON-RPC 2.0 aceita de harness.list. Lista as ferramentas ligadas e os apelidos de pasta. A versão 2 acrescenta os apelidos das pastas ausentes na partida, omitidos quando não há nenhuma. Só nomes; nunca inclui programa, argumentos, ambiente ou caminho.
///
/// `urn:ads:command:harness.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListResponseV2 {
    pub jsonrpc: CommandHarnessListResponseV2Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandHarnessListResponseV2Result,
}

/// `jsonrpc` in `urn:ads:command:harness.list.response:2`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandHarnessListResponseV2Jsonrpc;

impl CommandHarnessListResponseV2Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandHarnessListResponseV2Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandHarnessListResponseV2Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:harness.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandHarnessListResponseV2Result {
    pub harnesses: CommandHarnessListResponseV2ResultHarnesses,
    pub workdir_aliases: CommandHarnessListResponseV2ResultWorkdirAliases,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub missing_workdir_aliases: Option<CommandHarnessListResponseV2ResultMissingWorkdirAliases>,
}

/// `result.harnesses` in `urn:ads:command:harness.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct CommandHarnessListResponseV2ResultHarnesses(Vec<CommonIdV1>);

impl CommandHarnessListResponseV2ResultHarnesses {
    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandHarnessListResponseV2ResultHarnesses",
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

impl TryFrom<Vec<CommonIdV1>> for CommandHarnessListResponseV2ResultHarnesses {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandHarnessListResponseV2ResultHarnesses> for Vec<CommonIdV1> {
    fn from(value: CommandHarnessListResponseV2ResultHarnesses) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for CommandHarnessListResponseV2ResultHarnesses {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Apelidos disponíveis para abrir uma sessão.
///
/// `result.workdir_aliases` in `urn:ads:command:harness.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct CommandHarnessListResponseV2ResultWorkdirAliases(Vec<CommonIdV1>);

impl CommandHarnessListResponseV2ResultWorkdirAliases {
    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandHarnessListResponseV2ResultWorkdirAliases",
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

impl TryFrom<Vec<CommonIdV1>> for CommandHarnessListResponseV2ResultWorkdirAliases {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandHarnessListResponseV2ResultWorkdirAliases> for Vec<CommonIdV1> {
    fn from(value: CommandHarnessListResponseV2ResultWorkdirAliases) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for CommandHarnessListResponseV2ResultWorkdirAliases {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Apelidos indisponíveis porque a pasta estava ausente na partida.
///
/// `result.missing_workdir_aliases` in `urn:ads:command:harness.list.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct CommandHarnessListResponseV2ResultMissingWorkdirAliases(Vec<CommonIdV1>);

impl CommandHarnessListResponseV2ResultMissingWorkdirAliases {
    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if !items.is_empty() && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "CommandHarnessListResponseV2ResultMissingWorkdirAliases",
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

impl TryFrom<Vec<CommonIdV1>> for CommandHarnessListResponseV2ResultMissingWorkdirAliases {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<CommandHarnessListResponseV2ResultMissingWorkdirAliases> for Vec<CommonIdV1> {
    fn from(value: CommandHarnessListResponseV2ResultMissingWorkdirAliases) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for CommandHarnessListResponseV2ResultMissingWorkdirAliases {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
