// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.session-handoff.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::ContractError;

/// Passagem entre sessões. Passagem autorizada para uma nova sessão, com referências opacas aos artefatos mantidos pelo produto.
///
/// `urn:ads:record:session-handoff:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordSessionHandoffV1 {
    pub id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    pub target_session_id: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub previous_handoff_id: Option<CommonIdV1>,
    pub trigger: RecordSessionHandoffV1Trigger,
    pub lineage: CommonIdV1,
    pub number: RecordSessionHandoffV1Number,
    pub created_at: CommonTimestampV1,
    pub state_document_id: CommonIdV1,
    pub opening_prompt_id: CommonIdV1,
    pub consent: RecordSessionHandoffV1Consent,
}

/// `trigger` in `urn:ads:record:session-handoff:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordSessionHandoffV1Trigger {
    #[serde(rename = "context_limit")]
    ContextLimit,
    #[serde(rename = "account_switch")]
    AccountSwitch,
    #[serde(rename = "model_refusal")]
    ModelRefusal,
}

impl RecordSessionHandoffV1Trigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContextLimit => "context_limit",
            Self::AccountSwitch => "account_switch",
            Self::ModelRefusal => "model_refusal",
        }
    }
}

/// `number` in `urn:ads:record:session-handoff:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordSessionHandoffV1Number(u64);

impl RecordSessionHandoffV1Number {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (2..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordSessionHandoffV1Number",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordSessionHandoffV1Number {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordSessionHandoffV1Number> for u64 {
    fn from(value: RecordSessionHandoffV1Number) -> u64 {
        value.0
    }
}

impl Serialize for RecordSessionHandoffV1Number {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `consent` in `urn:ads:record:session-handoff:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordSessionHandoffV1Consent {
    pub user_id: CommonIdV1,
    pub channel: RecordSessionHandoffV1ConsentChannel,
}

/// `consent.channel` in `urn:ads:record:session-handoff:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordSessionHandoffV1ConsentChannel {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl RecordSessionHandoffV1ConsentChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}
