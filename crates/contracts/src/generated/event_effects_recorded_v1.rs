// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.effects.recorded.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use super::record_action_effects_v1::RecordActionEffectsV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_EFFECTS_RECORDED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("daemon")))]),
        ]),
    )]),
)];

/// Efeitos concretos registrados. O registro record.action-effects:1 que o ensaio de uma ação achou, gravado pelo núcleo logo depois do plano, no stream effects/<action_id>, uma vez só (seq 1). effects_hash é o SHA-256 da serialização do registro que o núcleo grava; a resposta do dono (answer.recorded:2) repete esse hash, e a execução só começa se o ensaio dela achar o mesmo registro.
///
/// `urn:ads:event:effects.recorded:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventEffectsRecordedV1Fields")]
pub struct EventEffectsRecordedV1(EventEffectsRecordedV1Fields);

/// The fields of [`EventEffectsRecordedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEffectsRecordedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventEffectsRecordedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventEffectsRecordedV1Stream,
    pub seq: EventEffectsRecordedV1Seq,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventEffectsRecordedV1Data,
}

impl EventEffectsRecordedV1 {
    pub fn new(fields: EventEffectsRecordedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventEffectsRecordedV1",
            &fields,
            EVENT_EFFECTS_RECORDED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventEffectsRecordedV1Fields {
        self.0
    }
}

impl Deref for EventEffectsRecordedV1 {
    type Target = EventEffectsRecordedV1Fields;

    fn deref(&self) -> &EventEffectsRecordedV1Fields {
        &self.0
    }
}

impl TryFrom<EventEffectsRecordedV1Fields> for EventEffectsRecordedV1 {
    type Error = ContractError;

    fn try_from(fields: EventEffectsRecordedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventEffectsRecordedV1> for EventEffectsRecordedV1Fields {
    fn from(value: EventEffectsRecordedV1) -> EventEffectsRecordedV1Fields {
        value.0
    }
}

impl Serialize for EventEffectsRecordedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:effects.recorded:1`.
///
/// Always `"urn:ads:event:effects.recorded:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventEffectsRecordedV1Type;

impl EventEffectsRecordedV1Type {
    pub const VALUE: &str = "urn:ads:event:effects.recorded:1";
}

impl Serialize for EventEffectsRecordedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventEffectsRecordedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_EFFECTS_RECORDED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_EFFECTS_RECORDED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^effects/");

/// `stream` in `urn:ads:event:effects.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventEffectsRecordedV1Stream(String);

impl EventEffectsRecordedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventEffectsRecordedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_EFFECTS_RECORDED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_EFFECTS_RECORDED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventEffectsRecordedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventEffectsRecordedV1Stream> for String {
    fn from(value: EventEffectsRecordedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventEffectsRecordedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventEffectsRecordedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `seq` in `urn:ads:event:effects.recorded:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventEffectsRecordedV1Seq;

impl EventEffectsRecordedV1Seq {
    pub const VALUE: i64 = 1;
}

impl Serialize for EventEffectsRecordedV1Seq {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventEffectsRecordedV1Seq {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:effects.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEffectsRecordedV1Data {
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub effects_hash: CommonSha256V1,
    pub effects: RecordActionEffectsV1,
}
