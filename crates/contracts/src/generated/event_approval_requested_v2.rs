// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.approval.requested.v2.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Aprovação pedida. Uma ação com risco pede aprovação: vale para o hash do plano e expira na validade indicada. A versão 2 acrescenta action_id, obrigatório, que amarra a aprovação à ação que a pediu.
///
/// `urn:ads:event:approval.requested:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventApprovalRequestedV2 {
    pub id: CommonUlidV1,
    pub r#type: EventApprovalRequestedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventApprovalRequestedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventApprovalRequestedV2Data,
}

/// `type` in `urn:ads:event:approval.requested:2`.
///
/// Always `"urn:ads:event:approval.requested:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventApprovalRequestedV2Type;

impl EventApprovalRequestedV2Type {
    pub const VALUE: &str = "urn:ads:event:approval.requested:2";
}

impl Serialize for EventApprovalRequestedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventApprovalRequestedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_APPROVAL_REQUESTED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_APPROVAL_REQUESTED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^approval/");

/// `stream` in `urn:ads:event:approval.requested:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventApprovalRequestedV2Stream(String);

impl EventApprovalRequestedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventApprovalRequestedV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_APPROVAL_REQUESTED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_APPROVAL_REQUESTED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventApprovalRequestedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventApprovalRequestedV2Stream> for String {
    fn from(value: EventApprovalRequestedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventApprovalRequestedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventApprovalRequestedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:approval.requested:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventApprovalRequestedV2Data {
    pub approval_id: CommonIdV1,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub risk: CommonRiskV1,
    pub summary: EventApprovalRequestedV2DataSummary,
    pub expires_at: CommonTimestampV1,
    pub needs_elevation: bool,
}

/// `data.summary` in `urn:ads:event:approval.requested:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventApprovalRequestedV2DataSummary(String);

impl EventApprovalRequestedV2DataSummary {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventApprovalRequestedV2DataSummary",
                text,
            ))
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

impl TryFrom<String> for EventApprovalRequestedV2DataSummary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventApprovalRequestedV2DataSummary> for String {
    fn from(value: EventApprovalRequestedV2DataSummary) -> String {
        value.0
    }
}

impl fmt::Display for EventApprovalRequestedV2DataSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventApprovalRequestedV2DataSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
