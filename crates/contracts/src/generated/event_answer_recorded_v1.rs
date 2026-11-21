// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.answer.recorded.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_ANSWER_RECORDED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Resposta do dono registrada. A resposta do dono a uma aprovação, gravada antes de o portão agir sobre ela: o aparelho (actor), o dono, o instante (ts), a decisão e o plano que o dono viu. Fica no stream answer/<approval_id>. A decisão do portão vem depois, em approval.granted:2 ou approval.denied:2, e pode ser uma negação mesmo quando o dono aprovou (política mudou).
///
/// `urn:ads:event:answer.recorded:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventAnswerRecordedV1Fields")]
pub struct EventAnswerRecordedV1(EventAnswerRecordedV1Fields);

/// The fields of [`EventAnswerRecordedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAnswerRecordedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventAnswerRecordedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventAnswerRecordedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventAnswerRecordedV1Data,
}

impl EventAnswerRecordedV1 {
    pub fn new(fields: EventAnswerRecordedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventAnswerRecordedV1",
            &fields,
            EVENT_ANSWER_RECORDED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventAnswerRecordedV1Fields {
        self.0
    }
}

impl Deref for EventAnswerRecordedV1 {
    type Target = EventAnswerRecordedV1Fields;

    fn deref(&self) -> &EventAnswerRecordedV1Fields {
        &self.0
    }
}

impl TryFrom<EventAnswerRecordedV1Fields> for EventAnswerRecordedV1 {
    type Error = ContractError;

    fn try_from(fields: EventAnswerRecordedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventAnswerRecordedV1> for EventAnswerRecordedV1Fields {
    fn from(value: EventAnswerRecordedV1) -> EventAnswerRecordedV1Fields {
        value.0
    }
}

impl Serialize for EventAnswerRecordedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:answer.recorded:1`.
///
/// Always `"urn:ads:event:answer.recorded:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventAnswerRecordedV1Type;

impl EventAnswerRecordedV1Type {
    pub const VALUE: &str = "urn:ads:event:answer.recorded:1";
}

impl Serialize for EventAnswerRecordedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventAnswerRecordedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ANSWER_RECORDED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ANSWER_RECORDED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^answer/");

/// `stream` in `urn:ads:event:answer.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventAnswerRecordedV1Stream(String);

impl EventAnswerRecordedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventAnswerRecordedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ANSWER_RECORDED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ANSWER_RECORDED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventAnswerRecordedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventAnswerRecordedV1Stream> for String {
    fn from(value: EventAnswerRecordedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventAnswerRecordedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventAnswerRecordedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:answer.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAnswerRecordedV1Data {
    pub approval_id: CommonIdV1,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub decision: EventAnswerRecordedV1DataDecision,
    pub owner_id: CommonIdV1,
    pub via: EventAnswerRecordedV1DataVia,
}

/// `data.decision` in `urn:ads:event:answer.recorded:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventAnswerRecordedV1DataDecision {
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "deny")]
    Deny,
}

impl EventAnswerRecordedV1DataDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }
}

/// `data.via` in `urn:ads:event:answer.recorded:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventAnswerRecordedV1DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl EventAnswerRecordedV1DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}
