// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.routing.proposed.v2.schema.json. Não edite à mão.

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

/// Rota proposta. O roteador propôs, para uma tarefa, a faixa, a ferramenta, o provedor e o modelo, citando a regra que usou. Proposta não é decisão: nada roda por causa deste evento. Inclui o apelido da conta, quando aplicável.
///
/// `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingProposedV2 {
    pub id: CommonUlidV1,
    pub r#type: EventRoutingProposedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventRoutingProposedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventRoutingProposedV2Data,
}

/// `type` in `urn:ads:event:routing.proposed:2`.
///
/// Always `"urn:ads:event:routing.proposed:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventRoutingProposedV2Type;

impl EventRoutingProposedV2Type {
    pub const VALUE: &str = "urn:ads:event:routing.proposed:2";
}

impl Serialize for EventRoutingProposedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventRoutingProposedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ROUTING_PROPOSED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ROUTING_PROPOSED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^routing/");

/// `stream` in `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingProposedV2Stream(String);

impl EventRoutingProposedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingProposedV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_PROPOSED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_ROUTING_PROPOSED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingProposedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingProposedV2Stream> for String {
    fn from(value: EventRoutingProposedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingProposedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingProposedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRoutingProposedV2Data {
    pub task_id: CommonIdV1,
    pub mode: EventRoutingProposedV2DataMode,
    #[serde(deserialize_with = "support::nullable")]
    pub rule_id: Option<CommonIdV1>,
    pub tier: EventRoutingProposedV2DataTier,
    pub harness: CommonIdV1,
    pub provider: CommonIdV1,
    pub model: EventRoutingProposedV2DataModel,
    #[serde(deserialize_with = "support::nullable")]
    pub effort: Option<EventRoutingProposedV2DataEffort>,
    #[serde(deserialize_with = "support::nullable")]
    pub account_alias: Option<CommonIdV1>,
}

/// `data.mode` in `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventRoutingProposedV2DataMode {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "propose")]
    Propose,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "shadow")]
    Shadow,
}

impl EventRoutingProposedV2DataMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Propose => "propose",
            Self::Auto => "auto",
            Self::Shadow => "shadow",
        }
    }
}

static EVENT_ROUTING_PROPOSED_V2_DATA_TIER_PATTERN: Pattern = Pattern::new(r"^T[0-3]$");

/// `data.tier` in `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingProposedV2DataTier(String);

impl EventRoutingProposedV2DataTier {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingProposedV2DataTier", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ROUTING_PROPOSED_V2_DATA_TIER_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingProposedV2DataTier {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingProposedV2DataTier> for String {
    fn from(value: EventRoutingProposedV2DataTier) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingProposedV2DataTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingProposedV2DataTier {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.model` in `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingProposedV2DataModel(String);

impl EventRoutingProposedV2DataModel {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingProposedV2DataModel", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=128).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingProposedV2DataModel {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingProposedV2DataModel> for String {
    fn from(value: EventRoutingProposedV2DataModel) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingProposedV2DataModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingProposedV2DataModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.effort` in `urn:ads:event:routing.proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventRoutingProposedV2DataEffort(String);

impl EventRoutingProposedV2DataEffort {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventRoutingProposedV2DataEffort", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=32).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventRoutingProposedV2DataEffort {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventRoutingProposedV2DataEffort> for String {
    fn from(value: EventRoutingProposedV2DataEffort) -> String {
        value.0
    }
}

impl fmt::Display for EventRoutingProposedV2DataEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventRoutingProposedV2DataEffort {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
