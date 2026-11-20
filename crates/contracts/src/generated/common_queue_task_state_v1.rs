// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.queue-task-state.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

/// Estado da tarefa da fila. Estados do ciclo de uma tarefa da fila contínua.
///
/// `urn:ads:common:queue-task-state:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommonQueueTaskStateV1 {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl CommonQueueTaskStateV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Review => "review",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}
