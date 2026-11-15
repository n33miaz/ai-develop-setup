// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.risk.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

/// Nível de risco. Nível de risco de uma ação ou tarefa: low, medium ou high.
///
/// `urn:ads:common:risk:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommonRiskV1 {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl CommonRiskV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}
