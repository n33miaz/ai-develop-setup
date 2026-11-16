// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.approval.denied.v2.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Aprovação negada. A aprovação foi negada para o hash do plano, com motivo opcional. A versão 2 acrescenta action_id, obrigatório: a ação para a qual a decisão foi tomada.
///
/// `urn:ads:event:approval.denied:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventApprovalDeniedV2 {
    pub id: CommonUlidV1,
    pub r#type: EventApprovalDeniedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventApprovalDeniedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventApprovalDeniedV2Data,
}

/// `type` in `urn:ads:event:approval.denied:2`.
///
/// Always `"urn:ads:event:approval.denied:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventApprovalDeniedV2Type;

impl EventApprovalDeniedV2Type {
    pub const VALUE: &str = "urn:ads:event:approval.denied:2";
}

impl Serialize for EventApprovalDeniedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventApprovalDeniedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_APPROVAL_DENIED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_APPROVAL_DENIED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^approval/");

/// `stream` in `urn:ads:event:approval.denied:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventApprovalDeniedV2Stream(String);

impl EventApprovalDeniedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventApprovalDeniedV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_APPROVAL_DENIED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_APPROVAL_DENIED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventApprovalDeniedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventApprovalDeniedV2Stream> for String {
    fn from(value: EventApprovalDeniedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventApprovalDeniedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventApprovalDeniedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:approval.denied:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventApprovalDeniedV2Data {
    pub approval_id: CommonIdV1,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    #[serde(deserialize_with = "support::nullable")]
    pub reason: Option<EventApprovalDeniedV2DataReason>,
}

/// `data.reason` in `urn:ads:event:approval.denied:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventApprovalDeniedV2DataReason(String);

impl EventApprovalDeniedV2DataReason {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventApprovalDeniedV2DataReason", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=500).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventApprovalDeniedV2DataReason {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventApprovalDeniedV2DataReason> for String {
    fn from(value: EventApprovalDeniedV2DataReason) -> String {
        value.0
    }
}

impl fmt::Display for EventApprovalDeniedV2DataReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventApprovalDeniedV2DataReason {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
