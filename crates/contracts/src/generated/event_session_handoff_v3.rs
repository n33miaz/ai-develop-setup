// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.handoff.v3.schema.json. Não edite à mão.

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

static EVENT_SESSION_HANDOFF_V3_RULES: &[(&str, Rule)] = &[(
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

/// Decisão de passagem entre sessões. Registra a proposta ou a decisão sobre uma passagem. Aceite e recusa exigem um aparelho como autor. O aceite de uma oferta de outro modelo nomeia a alternativa escolhida.
///
/// `urn:ads:event:session.handoff:3`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionHandoffV3Fields")]
pub struct EventSessionHandoffV3(EventSessionHandoffV3Fields);

/// The fields of [`EventSessionHandoffV3`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV3Fields {
    pub id: CommonUlidV1,
    pub r#type: EventSessionHandoffV3Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionHandoffV3Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionHandoffV3DataClass,
    pub data: EventSessionHandoffV3Data,
}

impl EventSessionHandoffV3 {
    pub fn new(fields: EventSessionHandoffV3Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionHandoffV3",
            &fields,
            EVENT_SESSION_HANDOFF_V3_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionHandoffV3Fields {
        self.0
    }
}

impl Deref for EventSessionHandoffV3 {
    type Target = EventSessionHandoffV3Fields;

    fn deref(&self) -> &EventSessionHandoffV3Fields {
        &self.0
    }
}

impl TryFrom<EventSessionHandoffV3Fields> for EventSessionHandoffV3 {
    type Error = ContractError;

    fn try_from(fields: EventSessionHandoffV3Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionHandoffV3> for EventSessionHandoffV3Fields {
    fn from(value: EventSessionHandoffV3) -> EventSessionHandoffV3Fields {
        value.0
    }
}

impl Serialize for EventSessionHandoffV3 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:session.handoff:3`.
///
/// Always `"urn:ads:event:session.handoff:3"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV3Type;

impl EventSessionHandoffV3Type {
    pub const VALUE: &str = "urn:ads:event:session.handoff:3";
}

impl Serialize for EventSessionHandoffV3Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV3Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_V3_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_HANDOFF_V3_STREAM_PATTERN_2: Pattern = Pattern::new(r"^handoff/");

/// `stream` in `urn:ads:event:session.handoff:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionHandoffV3Stream(String);

impl EventSessionHandoffV3Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionHandoffV3Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_HANDOFF_V3_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_HANDOFF_V3_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionHandoffV3Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionHandoffV3Stream> for String {
    fn from(value: EventSessionHandoffV3Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionHandoffV3Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionHandoffV3Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.handoff:3`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV3DataClass;

impl EventSessionHandoffV3DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionHandoffV3DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV3DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:session.handoff:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV3Data {
    pub handoff_id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    pub status: EventSessionHandoffV3DataStatus,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub alternative_id: Option<CommonIdV1>,
}

/// `data.status` in `urn:ads:event:session.handoff:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionHandoffV3DataStatus {
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "declined")]
    Declined,
}

impl EventSessionHandoffV3DataStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }
}
