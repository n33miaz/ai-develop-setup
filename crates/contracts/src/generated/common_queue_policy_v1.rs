// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.queue-policy.v1.schema.json. Não edite à mão.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::ContractError;

/// Limites da fila. Limites de admissão e prazo de confirmação da sessão pelo host.
///
/// `urn:ads:common:queue-policy:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommonQueuePolicyV1 {
    pub attempt_limit: CommonQueuePolicyV1AttemptLimit,
    pub admission_limit: NonZeroU64,
    pub window_seconds: NonZeroU64,
    pub window_admissions: NonZeroU64,
    pub manual_window_admissions: NonZeroU64,
    pub bug_rule_window_admissions: NonZeroU64,
    pub clock_skew_seconds: u64,
    pub clock_rewind_seconds: u64,
    pub host_confirm_seconds: NonZeroU64,
}

/// `attempt_limit` in `urn:ads:common:queue-policy:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct CommonQueuePolicyV1AttemptLimit(u16);

impl CommonQueuePolicyV1AttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "CommonQueuePolicyV1AttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for CommonQueuePolicyV1AttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<CommonQueuePolicyV1AttemptLimit> for u16 {
    fn from(value: CommonQueuePolicyV1AttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for CommonQueuePolicyV1AttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
