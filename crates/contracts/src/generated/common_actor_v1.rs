// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.actor.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

use super::common_id_v1::CommonIdV1;

/// Autor de um fato. Quem causou um evento ou uma decisão: um aparelho, um agente de IA ou o próprio daemon.
///
/// `urn:ads:common:actor:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonActorV1 {
    pub kind: CommonActorV1Kind,
    pub id: CommonIdV1,
}

/// `kind` in `urn:ads:common:actor:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommonActorV1Kind {
    #[serde(rename = "device")]
    Device,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "daemon")]
    Daemon,
}

impl CommonActorV1Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Agent => "agent",
            Self::Daemon => "daemon",
        }
    }
}
