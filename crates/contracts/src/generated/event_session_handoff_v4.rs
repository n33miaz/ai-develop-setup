// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.handoff.v4.schema.json. Não edite à mão.

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

static EVENT_SESSION_HANDOFF_V4_RULES: &[(&str, Rule)] = &[
    (
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
                    Rule::Required(&["status"]),
                ]),
            )]),
            then: Some(&Rule::Properties(&[
                (
                    "actor",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
                    ]),
                ),
                (
                    "data",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Required(&["channel"]),
                    ]),
                ),
            ])),
            otherwise: Some(&Rule::Properties(&[(
                "data",
                Rule::AllOf(&[
                    Rule::Type(&[JsonType::Object]),
                    Rule::Not(&Rule::Required(&["channel"])),
                ]),
            )])),
        },
    ),
    (
        "#/allOf/2/if",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Required(&["data"]),
                Rule::Properties(&[(
                    "data",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Required(&["trigger", "status"]),
                        Rule::Properties(&[
                            ("trigger", Rule::Const(Literal::Str("model_refusal"))),
                            ("status", Rule::Const(Literal::Str("accepted"))),
                        ]),
                    ]),
                )]),
            ]),
            then: Some(&Rule::Properties(&[(
                "data",
                Rule::AllOf(&[
                    Rule::Type(&[JsonType::Object]),
                    Rule::Required(&["alternative_id"]),
                ]),
            )])),
            otherwise: None,
        },
    ),
];

/// Decisão de passagem entre sessões. Registra a proposta ou a decisão sobre uma passagem, com o gatilho. Aceite e recusa exigem um aparelho como autor e o canal do consentimento. O aceite de uma oferta após recusa de modelo nomeia a alternativa escolhida.
///
/// `urn:ads:event:session.handoff:4`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionHandoffV4Fields")]
pub struct EventSessionHandoffV4(EventSessionHandoffV4Fields);

/// The fields of [`EventSessionHandoffV4`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV4Fields {
    pub id: CommonUlidV1,
    pub r#type: EventSessionHandoffV4Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionHandoffV4Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventSessionHandoffV4DataClass,
    pub data: EventSessionHandoffV4Data,
}

impl EventSessionHandoffV4 {
    pub fn new(fields: EventSessionHandoffV4Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionHandoffV4",
            &fields,
            EVENT_SESSION_HANDOFF_V4_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionHandoffV4Fields {
        self.0
    }
}

impl Deref for EventSessionHandoffV4 {
    type Target = EventSessionHandoffV4Fields;

    fn deref(&self) -> &EventSessionHandoffV4Fields {
        &self.0
    }
}

impl TryFrom<EventSessionHandoffV4Fields> for EventSessionHandoffV4 {
    type Error = ContractError;

    fn try_from(fields: EventSessionHandoffV4Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionHandoffV4> for EventSessionHandoffV4Fields {
    fn from(value: EventSessionHandoffV4) -> EventSessionHandoffV4Fields {
        value.0
    }
}

impl Serialize for EventSessionHandoffV4 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:session.handoff:4`.
///
/// Always `"urn:ads:event:session.handoff:4"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV4Type;

impl EventSessionHandoffV4Type {
    pub const VALUE: &str = "urn:ads:event:session.handoff:4";
}

impl Serialize for EventSessionHandoffV4Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV4Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_HANDOFF_V4_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_HANDOFF_V4_STREAM_PATTERN_2: Pattern = Pattern::new(r"^handoff/");

/// `stream` in `urn:ads:event:session.handoff:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionHandoffV4Stream(String);

impl EventSessionHandoffV4Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionHandoffV4Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_HANDOFF_V4_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_HANDOFF_V4_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionHandoffV4Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionHandoffV4Stream> for String {
    fn from(value: EventSessionHandoffV4Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionHandoffV4Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionHandoffV4Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:session.handoff:4`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionHandoffV4DataClass;

impl EventSessionHandoffV4DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventSessionHandoffV4DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionHandoffV4DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:session.handoff:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionHandoffV4Data {
    pub handoff_id: CommonIdV1,
    pub source_session_id: CommonIdV1,
    pub trigger: EventSessionHandoffV4DataTrigger,
    pub status: EventSessionHandoffV4DataStatus,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub channel: Option<EventSessionHandoffV4DataChannel>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub alternative_id: Option<CommonIdV1>,
}

/// Gatilho da passagem, com os mesmos valores do registro.
///
/// `data.trigger` in `urn:ads:event:session.handoff:4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionHandoffV4DataTrigger {
    #[serde(rename = "context_limit")]
    ContextLimit,
    #[serde(rename = "account_switch")]
    AccountSwitch,
    #[serde(rename = "model_refusal")]
    ModelRefusal,
}

impl EventSessionHandoffV4DataTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContextLimit => "context_limit",
            Self::AccountSwitch => "account_switch",
            Self::ModelRefusal => "model_refusal",
        }
    }
}

/// `data.status` in `urn:ads:event:session.handoff:4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionHandoffV4DataStatus {
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "declined")]
    Declined,
}

impl EventSessionHandoffV4DataStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }
}

/// Canal pelo qual o aparelho aceitou ou recusou. Ausente na proposta.
///
/// `data.channel` in `urn:ads:event:session.handoff:4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionHandoffV4DataChannel {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl EventSessionHandoffV4DataChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}
