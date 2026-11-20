// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.task-queue.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use crate::support;

/// Fila contínua. Controle da fila. Toda fila nasce parada; só o dono autenticado pode ativá-la.
///
/// `urn:ads:record:task-queue:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTaskQueueV1 {
    pub version: RecordTaskQueueV1Version,
    pub id: CommonIdV1,
    pub owner_id: CommonIdV1,
    pub state: RecordTaskQueueV1State,
}

/// `version` in `urn:ads:record:task-queue:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordTaskQueueV1Version;

impl RecordTaskQueueV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordTaskQueueV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordTaskQueueV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `state` in `urn:ads:record:task-queue:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordTaskQueueV1State {
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "enabled")]
    Enabled,
}

impl RecordTaskQueueV1State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Enabled => "enabled",
        }
    }
}
