// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.get.response.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;

/// Resposta de queue.get. Resposta JSON-RPC 2.0 aceita de queue.get: a fila pedida e o estado dela no núcleo. A recusa volta como command.response:2 com error.
///
/// `urn:ads:command:queue.get.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueGetResponseV1 {
    pub jsonrpc: CommandQueueGetResponseV1Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandQueueGetResponseV1Result,
}

/// `jsonrpc` in `urn:ads:command:queue.get.response:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueGetResponseV1Jsonrpc;

impl CommandQueueGetResponseV1Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueGetResponseV1Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueGetResponseV1Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:queue.get.response:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueGetResponseV1Result {
    pub queue_id: CommonIdV1,
    pub state: CommandQueueGetResponseV1ResultState,
}

/// `result.state` in `urn:ads:command:queue.get.response:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandQueueGetResponseV1ResultState {
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "enabled")]
    Enabled,
}

impl CommandQueueGetResponseV1ResultState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Enabled => "enabled",
        }
    }
}
