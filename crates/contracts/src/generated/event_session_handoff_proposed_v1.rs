// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.handoff_proposed.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Passagem proposta por limiar de contexto. O núcleo propõe passar o trabalho a uma sessão nova porque o contexto da sessão atingiu o limiar configurado. Só metadados: nunca leva conteúdo da conversa. A decisão do dono é registrada em session.handoff.
///
/// `urn:ads:event:session.handoff_proposed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffProposedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionHandoffProposedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionHandoffProposedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionHandoffProposedV1DataClass,
    pub data: EventSessionHandoffProposedV1Data,
}

/// `type` in `urn:ads:event:session.handoff_proposed:1`.
///
/// Always `"urn:ads:event:session.handoff_proposed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffProposedV1Type;

impl EventSessionHandoffProposedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.handoff_proposed:1";
}

impl Serialize for EventSessionHandoffProposedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffProposedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_PROPOSED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_HANDOFF_PROPOSED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.handoff_proposed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionHandoffProposedV1Stream(String);

impl EventSessionHandoffProposedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV1Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_HANDOFF_PROPOSED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_HANDOFF_PROPOSED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionHandoffProposedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionHandoffProposedV1Stream> for String {
    fn from(value: EventSessionHandoffProposedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionHandoffProposedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionHandoffProposedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.handoff_proposed:1`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffProposedV1DataClass;

impl EventSessionHandoffProposedV1DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionHandoffProposedV1DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffProposedV1DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:session.handoff_proposed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffProposedV1Data {
    pub session_id: CommonIdV1,
    pub threshold_pct: EventSessionHandoffProposedV1DataThresholdPct,
    pub context_pct: EventSessionHandoffProposedV1DataContextPct,
    pub number: EventSessionHandoffProposedV1DataNumber,
    #[serde(deserialize_with = "support::nullable")]
    pub previous_document_id: Option<CommonIdV1>,
}

/// Limiar configurado que foi atingido, em percentual da janela de contexto.
///
/// `data.threshold_pct` in `urn:ads:event:session.handoff_proposed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionHandoffProposedV1DataThresholdPct(u16);

impl EventSessionHandoffProposedV1DataThresholdPct {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV1DataThresholdPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionHandoffProposedV1DataThresholdPct {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionHandoffProposedV1DataThresholdPct> for u16 {
    fn from(value: EventSessionHandoffProposedV1DataThresholdPct) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV1DataThresholdPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Percentual medido da janela de contexto ocupada, arredondado para baixo.
///
/// `data.context_pct` in `urn:ads:event:session.handoff_proposed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionHandoffProposedV1DataContextPct(u16);

impl EventSessionHandoffProposedV1DataContextPct {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV1DataContextPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionHandoffProposedV1DataContextPct {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionHandoffProposedV1DataContextPct> for u16 {
    fn from(value: EventSessionHandoffProposedV1DataContextPct) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV1DataContextPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Número N da sessão atual na cadeia; a primeira sessão é 1.
///
/// `data.number` in `urn:ads:event:session.handoff_proposed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct EventSessionHandoffProposedV1DataNumber(u64);

impl EventSessionHandoffProposedV1DataNumber {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV1DataNumber",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for EventSessionHandoffProposedV1DataNumber {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionHandoffProposedV1DataNumber> for u64 {
    fn from(value: EventSessionHandoffProposedV1DataNumber) -> u64 {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV1DataNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
