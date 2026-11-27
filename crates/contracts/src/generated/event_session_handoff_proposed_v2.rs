// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.handoff_proposed.v2.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

/// Passagem proposta por limiar de contexto. O núcleo propõe passar o trabalho a uma sessão nova porque o contexto da sessão atingiu o limiar configurado. Só metadados: nunca leva conteúdo da conversa. A decisão do dono é registrada em session.handoff.
///
/// `urn:ads:event:session.handoff_proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffProposedV2 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionHandoffProposedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionHandoffProposedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionHandoffProposedV2DataClass,
    pub data: EventSessionHandoffProposedV2Data,
}

/// `type` in `urn:ads:event:session.handoff_proposed:2`.
///
/// Always `"urn:ads:event:session.handoff_proposed:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffProposedV2Type;

impl EventSessionHandoffProposedV2Type {
    pub const VALUE: &str = "urn:ads:event:session.handoff_proposed:2";
}

impl Serialize for EventSessionHandoffProposedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffProposedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_PROPOSED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_HANDOFF_PROPOSED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.handoff_proposed:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionHandoffProposedV2Stream(String);

impl EventSessionHandoffProposedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV2Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_HANDOFF_PROPOSED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_HANDOFF_PROPOSED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionHandoffProposedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionHandoffProposedV2Stream> for String {
    fn from(value: EventSessionHandoffProposedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionHandoffProposedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionHandoffProposedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.handoff_proposed:2`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffProposedV2DataClass;

impl EventSessionHandoffProposedV2DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionHandoffProposedV2DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffProposedV2DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_PROPOSED_V2_DATA_RULES: &[(&str, Rule)] = &[(
    "#/properties/data/if",
    Rule::If {
        when: &Rule::Properties(&[("number", Rule::Const(Literal::Int(1)))]),
        then: Some(&Rule::Properties(&[(
            "previous_document_id",
            Rule::Type(&[JsonType::Null]),
        )])),
        otherwise: Some(&Rule::Properties(&[(
            "previous_document_id",
            Rule::Type(&[JsonType::String]),
        )])),
    },
)];

/// `data` in `urn:ads:event:session.handoff_proposed:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionHandoffProposedV2DataFields")]
pub struct EventSessionHandoffProposedV2Data(EventSessionHandoffProposedV2DataFields);

/// The fields of [`EventSessionHandoffProposedV2Data`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffProposedV2DataFields {
    pub session_id: CommonIdV1,
    pub threshold_pct: EventSessionHandoffProposedV2DataThresholdPct,
    pub context_pct: EventSessionHandoffProposedV2DataContextPct,
    pub number: EventSessionHandoffProposedV2DataNumber,
    #[serde(deserialize_with = "support::nullable")]
    pub previous_document_id: Option<CommonIdV1>,
}

impl EventSessionHandoffProposedV2Data {
    pub fn new(fields: EventSessionHandoffProposedV2DataFields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionHandoffProposedV2Data",
            &fields,
            EVENT_SESSION_HANDOFF_PROPOSED_V2_DATA_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionHandoffProposedV2DataFields {
        self.0
    }
}

impl Deref for EventSessionHandoffProposedV2Data {
    type Target = EventSessionHandoffProposedV2DataFields;

    fn deref(&self) -> &EventSessionHandoffProposedV2DataFields {
        &self.0
    }
}

impl TryFrom<EventSessionHandoffProposedV2DataFields> for EventSessionHandoffProposedV2Data {
    type Error = ContractError;

    fn try_from(fields: EventSessionHandoffProposedV2DataFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionHandoffProposedV2Data> for EventSessionHandoffProposedV2DataFields {
    fn from(value: EventSessionHandoffProposedV2Data) -> EventSessionHandoffProposedV2DataFields {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV2Data {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Limiar configurado que foi atingido, em percentual da janela de contexto.
///
/// `data.threshold_pct` in `urn:ads:event:session.handoff_proposed:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionHandoffProposedV2DataThresholdPct(u16);

impl EventSessionHandoffProposedV2DataThresholdPct {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV2DataThresholdPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionHandoffProposedV2DataThresholdPct {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionHandoffProposedV2DataThresholdPct> for u16 {
    fn from(value: EventSessionHandoffProposedV2DataThresholdPct) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV2DataThresholdPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Percentual medido da janela de contexto ocupada, arredondado para baixo.
///
/// `data.context_pct` in `urn:ads:event:session.handoff_proposed:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionHandoffProposedV2DataContextPct(u16);

impl EventSessionHandoffProposedV2DataContextPct {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV2DataContextPct",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionHandoffProposedV2DataContextPct {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionHandoffProposedV2DataContextPct> for u16 {
    fn from(value: EventSessionHandoffProposedV2DataContextPct) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV2DataContextPct {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Número N da sessão atual na cadeia; a primeira sessão é 1.
///
/// `data.number` in `urn:ads:event:session.handoff_proposed:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct EventSessionHandoffProposedV2DataNumber(u64);

impl EventSessionHandoffProposedV2DataNumber {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionHandoffProposedV2DataNumber",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for EventSessionHandoffProposedV2DataNumber {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionHandoffProposedV2DataNumber> for u64 {
    fn from(value: EventSessionHandoffProposedV2DataNumber) -> u64 {
        value.0
    }
}

impl Serialize for EventSessionHandoffProposedV2DataNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
