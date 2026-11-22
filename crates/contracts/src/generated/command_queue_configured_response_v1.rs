// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.configured.response.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;

/// Resposta de queue.configured. Resposta JSON-RPC 2.0 aceita de queue.configured: o id da fila que o núcleo serve, para a interface não depender do arquivo de identidades. Não diz se a fila já foi criada; queue.get diz. A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:queue.configured.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueConfiguredResponseV1 {
    pub jsonrpc: CommandQueueConfiguredResponseV1Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandQueueConfiguredResponseV1Result,
}

/// `jsonrpc` in `urn:ads:command:queue.configured.response:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueConfiguredResponseV1Jsonrpc;

impl CommandQueueConfiguredResponseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueConfiguredResponseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueConfiguredResponseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:queue.configured.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueConfiguredResponseV1Result {
    pub queue_id: CommonIdV1,
}
