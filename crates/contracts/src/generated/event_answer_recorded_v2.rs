// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.answer.recorded.v2.schema.json. Não edite à mão.

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

static EVENT_ANSWER_RECORDED_V2_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Resposta do dono registrada. A resposta do dono a uma aprovação, gravada antes de o portão agir sobre ela: o aparelho (actor), o dono, o instante (ts), a decisão, o plano que o dono viu, o hash do registro de efeitos concretos que a listagem mostrou (effects_hash, null quando a ação não tem esse registro) e a chave de idempotência do comando, que deduplica a resposta no log. Fica no stream answer/<approval_id>. A decisão do portão vem depois, em approval.granted:2 ou approval.denied:2, e pode ser uma negação mesmo quando o dono aprovou (política mudou). A versão 2 acrescenta effects_hash, obrigatório.
///
/// `urn:ads:event:answer.recorded:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventAnswerRecordedV2Fields")]
pub struct EventAnswerRecordedV2(EventAnswerRecordedV2Fields);

/// The fields of [`EventAnswerRecordedV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAnswerRecordedV2Fields {
    pub id: CommonUlidV1,
    pub r#type: EventAnswerRecordedV2Type,
    pub ts: CommonTimestampV1,
    pub stream: EventAnswerRecordedV2Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventAnswerRecordedV2Data,
}

impl EventAnswerRecordedV2 {
    pub fn new(fields: EventAnswerRecordedV2Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventAnswerRecordedV2",
            &fields,
            EVENT_ANSWER_RECORDED_V2_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventAnswerRecordedV2Fields {
        self.0
    }
}

impl Deref for EventAnswerRecordedV2 {
    type Target = EventAnswerRecordedV2Fields;

    fn deref(&self) -> &EventAnswerRecordedV2Fields {
        &self.0
    }
}

impl TryFrom<EventAnswerRecordedV2Fields> for EventAnswerRecordedV2 {
    type Error = ContractError;

    fn try_from(fields: EventAnswerRecordedV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventAnswerRecordedV2> for EventAnswerRecordedV2Fields {
    fn from(value: EventAnswerRecordedV2) -> EventAnswerRecordedV2Fields {
        value.0
    }
}

impl Serialize for EventAnswerRecordedV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:answer.recorded:2`.
///
/// Always `"urn:ads:event:answer.recorded:2"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventAnswerRecordedV2Type;

impl EventAnswerRecordedV2Type {
    pub const VALUE: &str = "urn:ads:event:answer.recorded:2";
}

impl Serialize for EventAnswerRecordedV2Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventAnswerRecordedV2Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ANSWER_RECORDED_V2_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ANSWER_RECORDED_V2_STREAM_PATTERN_2: Pattern = Pattern::new(r"^answer/");

/// `stream` in `urn:ads:event:answer.recorded:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventAnswerRecordedV2Stream(String);

impl EventAnswerRecordedV2Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventAnswerRecordedV2Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ANSWER_RECORDED_V2_STREAM_PATTERN_1.matches(text)
            && EVENT_ANSWER_RECORDED_V2_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventAnswerRecordedV2Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventAnswerRecordedV2Stream> for String {
    fn from(value: EventAnswerRecordedV2Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventAnswerRecordedV2Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventAnswerRecordedV2Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:answer.recorded:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAnswerRecordedV2Data {
    pub approval_id: CommonIdV1,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    #[serde(deserialize_with = "support::nullable")]
    pub effects_hash: Option<CommonSha256V1>,
    pub decision: EventAnswerRecordedV2DataDecision,
    pub owner_id: CommonIdV1,
    pub via: EventAnswerRecordedV2DataVia,
    pub idempotency_key: CommonUlidV1,
}

/// `data.decision` in `urn:ads:event:answer.recorded:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventAnswerRecordedV2DataDecision {
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "deny")]
    Deny,
}

impl EventAnswerRecordedV2DataDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }
}

/// `data.via` in `urn:ads:event:answer.recorded:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventAnswerRecordedV2DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl EventAnswerRecordedV2DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}
