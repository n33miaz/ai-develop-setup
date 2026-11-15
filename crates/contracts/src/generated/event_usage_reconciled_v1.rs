// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.usage.reconciled.v1.schema.json. Não edite à mão.

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

/// Uso reconciliado. Duas fontes mediram o mesmo turno; ficou um registro e as outras foram descartadas pela chave de deduplicação.
///
/// `urn:ads:event:usage.reconciled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventUsageReconciledV1 {
    pub id: CommonUlidV1,
    pub r#type: EventUsageReconciledV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventUsageReconciledV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventUsageReconciledV1Data,
}

/// `type` in `urn:ads:event:usage.reconciled:1`.
///
/// Always `"urn:ads:event:usage.reconciled:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventUsageReconciledV1Type;

impl EventUsageReconciledV1Type {
    pub const VALUE: &str = "urn:ads:event:usage.reconciled:1";
}

impl Serialize for EventUsageReconciledV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventUsageReconciledV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_USAGE_RECONCILED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_USAGE_RECONCILED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^usage/");

/// `stream` in `urn:ads:event:usage.reconciled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventUsageReconciledV1Stream(String);

impl EventUsageReconciledV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventUsageReconciledV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_USAGE_RECONCILED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_USAGE_RECONCILED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventUsageReconciledV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventUsageReconciledV1Stream> for String {
    fn from(value: EventUsageReconciledV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventUsageReconciledV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventUsageReconciledV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:usage.reconciled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventUsageReconciledV1Data {
    pub dedupe_key: EventUsageReconciledV1DataDedupeKey,
    pub kept_event_id: CommonIdV1,
    pub dropped_sources: EventUsageReconciledV1DataDroppedSources,
}

/// `data.dedupe_key` in `urn:ads:event:usage.reconciled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventUsageReconciledV1DataDedupeKey(String);

impl EventUsageReconciledV1DataDedupeKey {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventUsageReconciledV1DataDedupeKey",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=256).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventUsageReconciledV1DataDedupeKey {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventUsageReconciledV1DataDedupeKey> for String {
    fn from(value: EventUsageReconciledV1DataDedupeKey) -> String {
        value.0
    }
}

impl fmt::Display for EventUsageReconciledV1DataDedupeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventUsageReconciledV1DataDedupeKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.dropped_sources` in `urn:ads:event:usage.reconciled:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<EventUsageReconciledV1DataDroppedSourcesItem>")]
pub struct EventUsageReconciledV1DataDroppedSources(
    Vec<EventUsageReconciledV1DataDroppedSourcesItem>,
);

impl EventUsageReconciledV1DataDroppedSources {
    pub fn new(
        items: Vec<EventUsageReconciledV1DataDroppedSourcesItem>,
    ) -> Result<Self, ContractError> {
        if !items.is_empty() && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "EventUsageReconciledV1DataDroppedSources",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[EventUsageReconciledV1DataDroppedSourcesItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<EventUsageReconciledV1DataDroppedSourcesItem> {
        self.0
    }
}

impl TryFrom<Vec<EventUsageReconciledV1DataDroppedSourcesItem>>
    for EventUsageReconciledV1DataDroppedSources
{
    type Error = ContractError;

    fn try_from(
        items: Vec<EventUsageReconciledV1DataDroppedSourcesItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<EventUsageReconciledV1DataDroppedSources>
    for Vec<EventUsageReconciledV1DataDroppedSourcesItem>
{
    fn from(
        value: EventUsageReconciledV1DataDroppedSources,
    ) -> Vec<EventUsageReconciledV1DataDroppedSourcesItem> {
        value.0
    }
}

impl Serialize for EventUsageReconciledV1DataDroppedSources {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.dropped_sources[]` in `urn:ads:event:usage.reconciled:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventUsageReconciledV1DataDroppedSourcesItem {
    #[serde(rename = "adapter")]
    Adapter,
    #[serde(rename = "cli_json")]
    CliJson,
    #[serde(rename = "statusline")]
    Statusline,
    #[serde(rename = "transcript")]
    Transcript,
    #[serde(rename = "rollout")]
    Rollout,
    #[serde(rename = "usagebar")]
    Usagebar,
    #[serde(rename = "manual")]
    Manual,
}

impl EventUsageReconciledV1DataDroppedSourcesItem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adapter => "adapter",
            Self::CliJson => "cli_json",
            Self::Statusline => "statusline",
            Self::Transcript => "transcript",
            Self::Rollout => "rollout",
            Self::Usagebar => "usagebar",
            Self::Manual => "manual",
        }
    }
}
