// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/command.queue.get.response.v2.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_queue_policy_v1::CommonQueuePolicyV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;

/// Resposta de queue.get. Resposta aceita de queue.get: estado da fila e política vigente registrada no histórico. A política é null se nunca foi registrada.
///
/// `urn:ads:command:queue.get.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueGetResponseV2 {
    pub jsonrpc: CommandQueueGetResponseV2Jsonrpc,
    pub id: CommonUlidV1,
    pub result: CommandQueueGetResponseV2Result,
}

/// `jsonrpc` in `urn:ads:command:queue.get.response:2`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CommandQueueGetResponseV2Jsonrpc;

impl CommandQueueGetResponseV2Jsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for CommandQueueGetResponseV2Jsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for CommandQueueGetResponseV2Jsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `result` in `urn:ads:command:queue.get.response:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandQueueGetResponseV2Result {
    pub queue_id: CommonIdV1,
    pub state: CommandQueueGetResponseV2ResultState,
    #[serde(deserialize_with = "support::nullable")]
    pub policy: Option<CommonQueuePolicyV1>,
}

/// `result.state` in `urn:ads:command:queue.get.response:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandQueueGetResponseV2ResultState {
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "enabled")]
    Enabled,
}

impl CommandQueueGetResponseV2ResultState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Enabled => "enabled",
        }
    }
}
