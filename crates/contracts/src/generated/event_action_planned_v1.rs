// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.action.planned.v1.schema.json. Não edite à mão.

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

/// Ação planejada. O ensaio (dry-run) de uma ação produziu um plano: o verbo, o risco, o hash do plano e se pede elevação. Nada foi alterado ainda; a aprovação e a execução valem só para este hash.
///
/// `urn:ads:event:action.planned:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventActionPlannedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventActionPlannedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventActionPlannedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventActionPlannedV1Data,
}

/// `type` in `urn:ads:event:action.planned:1`.
///
/// Always `"urn:ads:event:action.planned:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventActionPlannedV1Type;

impl EventActionPlannedV1Type {
    pub const VALUE: &str = "urn:ads:event:action.planned:1";
}

impl Serialize for EventActionPlannedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventActionPlannedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ACTION_PLANNED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ACTION_PLANNED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^action/");

/// `stream` in `urn:ads:event:action.planned:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventActionPlannedV1Stream(String);

impl EventActionPlannedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventActionPlannedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ACTION_PLANNED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ACTION_PLANNED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventActionPlannedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventActionPlannedV1Stream> for String {
    fn from(value: EventActionPlannedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventActionPlannedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventActionPlannedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:action.planned:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventActionPlannedV1Data {
    pub action_id: CommonIdV1,
    pub verb: EventActionPlannedV1DataVerb,
    pub risk: CommonRiskV1,
    pub plan_hash: CommonSha256V1,
    pub needs_elevation: bool,
}

static EVENT_ACTION_PLANNED_V1_DATA_VERB_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$");

/// `data.verb` in `urn:ads:event:action.planned:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventActionPlannedV1DataVerb(String);

impl EventActionPlannedV1DataVerb {
    pub const MAX_CHARS: usize = 64;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventActionPlannedV1DataVerb", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= Self::MAX_CHARS && EVENT_ACTION_PLANNED_V1_DATA_VERB_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventActionPlannedV1DataVerb {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventActionPlannedV1DataVerb> for String {
    fn from(value: EventActionPlannedV1DataVerb) -> String {
        value.0
    }
}

impl fmt::Display for EventActionPlannedV1DataVerb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventActionPlannedV1DataVerb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
