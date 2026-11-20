// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.handoff.v2.schema.json. Não edite à mão.

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

static EVENT_SESSION_HANDOFF_V2_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/if",
    Rule::If {
        when: &Rule::Properties(&[(
            "data",
            Rule::AllOf(&[
                Rule::Type(&[JsonType::Object]),
                Rule::Properties(&[(
                    "status",
                    Rule::Enum(&[Literal::Str("accepted"), Literal::Str("declined")]),
                )]),
            ]),
        )]),
        then: Some(&Rule::Properties(&[(
            "actor",
            Rule::AllOf(&[
                Rule::Type(&[JsonType::Object]),
                Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
            ]),
        )])),
        otherwise: None,
    },
)];

/// Decisão de passagem entre sessões. Registra a proposta ou a decisão sobre uma passagem. Aceite e recusa exigem um aparelho como autor.
///
/// `urn:ads:event:session.handoff:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionHandoffV2Fields")]
pub struct EventSessionHandoffV2(EventSessionHandoffV2Fields);

/// The fields of [`EventSessionHandoffV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV2Fields {
    pub id: CommonUlidV1,
    pub r#type: EventSessionHandoffV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionHandoffV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionHandoffV2DataClass,
    pub data: EventSessionHandoffV2Data,
}

impl EventSessionHandoffV2 {
    pub fn new(fields: EventSessionHandoffV2Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionHandoffV2",
            &fields,
            EVENT_SESSION_HANDOFF_V2_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionHandoffV2Fields {
        self.0
    }
}

impl Deref for EventSessionHandoffV2 {
    type Target = EventSessionHandoffV2Fields;

    fn deref(&self) -> &EventSessionHandoffV2Fields {
        &self.0
    }
}

impl TryFrom<EventSessionHandoffV2Fields> for EventSessionHandoffV2 {
    type Error = ContractError;

    fn try_from(fields: EventSessionHandoffV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionHandoffV2> for EventSessionHandoffV2Fields {
    fn from(value: EventSessionHandoffV2) -> EventSessionHandoffV2Fields {
        value.0
    }
}

impl Serialize for EventSessionHandoffV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:session.handoff:2`.
///
/// Always `"urn:ads:event:session.handoff:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV2Type;

impl EventSessionHandoffV2Type {
    pub const VALUE: &str = "urn:ads:event:session.handoff:2";
}

impl Serialize for EventSessionHandoffV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_HANDOFF_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^handoff/");

/// `stream` in `urn:ads:event:session.handoff:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionHandoffV2Stream(String);

impl EventSessionHandoffV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionHandoffV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_HANDOFF_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_HANDOFF_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionHandoffV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionHandoffV2Stream> for String {
    fn from(value: EventSessionHandoffV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionHandoffV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionHandoffV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.handoff:2`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV2DataClass;

impl EventSessionHandoffV2DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionHandoffV2DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV2DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:session.handoff:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV2Data {
    pub handoff_id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    pub status: EventSessionHandoffV2DataStatus,
}

/// `data.status` in `urn:ads:event:session.handoff:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionHandoffV2DataStatus {
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "declined")]
    Declined,
}

impl EventSessionHandoffV2DataStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }
}
