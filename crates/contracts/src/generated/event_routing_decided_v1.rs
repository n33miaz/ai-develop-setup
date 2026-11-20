// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.routing.decided.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Rota decidida. A rota em vigor de uma tarefa foi fixada: pelo dono (modo manual, ou aceite de uma proposta) ou por regra (modo auto). O autor do evento diz quem decidiu.
///
/// `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingDecidedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventRoutingDecidedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventRoutingDecidedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventRoutingDecidedV1Data,
}

/// `type` in `urn:ads:event:routing.decided:1`.
///
/// Always `"urn:ads:event:routing.decided:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventRoutingDecidedV1Type;

impl EventRoutingDecidedV1Type {
    pub const VALUE: &str = "urn:ads:event:routing.decided:1";
}

impl Serialize for EventRoutingDecidedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventRoutingDecidedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ROUTING_DECIDED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ROUTING_DECIDED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^routing/");

/// `stream` in `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingDecidedV1Stream(String);

impl EventRoutingDecidedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingDecidedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_DECIDED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ROUTING_DECIDED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingDecidedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingDecidedV1Stream> for String {
    fn from(value: EventRoutingDecidedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingDecidedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingDecidedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingDecidedV1Data {
    pub task_id: CommonIdV1,
    pub mode: EventRoutingDecidedV1DataMode,
    #[serde(deserialize_with = "support::nullable")]
    pub rule_id: Option<CommonIdV1>,
    pub tier: EventRoutingDecidedV1DataTier,
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingDecidedV1DataModel,
    #[serde(deserialize_with = "support::nullable")]
    pub effort: Option<EventRoutingDecidedV1DataEffort>,
}

/// `data.mode` in `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventRoutingDecidedV1DataMode {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "propose")]
    Propose,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "shadow")]
    Shadow,
}

impl EventRoutingDecidedV1DataMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Propose => "propose",
            Self::Auto => "auto",
            Self::Shadow => "shadow",
        }
    }
}

static EVENT_ROUTING_DECIDED_V1_DATA_TIER_PATTERN: Pattern = Pattern::new(r"^T[0-3]$");

/// `data.tier` in `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingDecidedV1DataTier(String);

impl EventRoutingDecidedV1DataTier {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingDecidedV1DataTier", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_DECIDED_V1_DATA_TIER_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingDecidedV1DataTier {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingDecidedV1DataTier> for String {
    fn from(value: EventRoutingDecidedV1DataTier) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingDecidedV1DataTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingDecidedV1DataTier {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.model` in `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingDecidedV1DataModel(String);

impl EventRoutingDecidedV1DataModel {
    pub const MAX_CHARS: usize = 128;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingDecidedV1DataModel", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingDecidedV1DataModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingDecidedV1DataModel> for String {
    fn from(value: EventRoutingDecidedV1DataModel) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingDecidedV1DataModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingDecidedV1DataModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.effort` in `urn:ads:event:routing.decided:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingDecidedV1DataEffort(String);

impl EventRoutingDecidedV1DataEffort {
    pub const MAX_CHARS: usize = 32;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingDecidedV1DataEffort", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingDecidedV1DataEffort {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingDecidedV1DataEffort> for String {
    fn from(value: EventRoutingDecidedV1DataEffort) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingDecidedV1DataEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingDecidedV1DataEffort {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
