// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.effects.refused.v1.schema.json. Não edite à mão.

use std::fmt;
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

static EVENT_EFFECTS_REFUSED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("daemon")))]),
        ]),
    )]),
)];

/// Ensaio de efeitos recusado. O ensaio de uma ação que pede aprovação não gerou registro de efeitos: o executor recusou o ensaio (por exemplo E_FS_EFFECTS_TOO_LARGE, acima de max_effects_bytes, ou E_FS_PLAN_ENTRY_LIMIT) ou o núcleo não conseguiu gravar o registro (E_EFFECTS_UNRECORDABLE). Gravado pelo núcleo no lugar de effects.recorded:1, no stream effects/<action_id>, uma vez só (seq 1). A ação continua sem poder ser aprovada pela API onde precisa de efeitos, e a listagem mostra o código.
///
/// `urn:ads:event:effects.refused:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventEffectsRefusedV1Fields")]
pub struct EventEffectsRefusedV1(EventEffectsRefusedV1Fields);

/// The fields of [`EventEffectsRefusedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEffectsRefusedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventEffectsRefusedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventEffectsRefusedV1Stream,
    pub seq: EventEffectsRefusedV1Seq,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventEffectsRefusedV1Data,
}

impl EventEffectsRefusedV1 {
    pub fn new(fields: EventEffectsRefusedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventEffectsRefusedV1",
            &fields,
            EVENT_EFFECTS_REFUSED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventEffectsRefusedV1Fields {
        self.0
    }
}

impl Deref for EventEffectsRefusedV1 {
    type Target = EventEffectsRefusedV1Fields;

    fn deref(&self) -> &EventEffectsRefusedV1Fields {
        &self.0
    }
}

impl TryFrom<EventEffectsRefusedV1Fields> for EventEffectsRefusedV1 {
    type Error = ContractError;

    fn try_from(fields: EventEffectsRefusedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventEffectsRefusedV1> for EventEffectsRefusedV1Fields {
    fn from(value: EventEffectsRefusedV1) -> EventEffectsRefusedV1Fields {
        value.0
    }
}

impl Serialize for EventEffectsRefusedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:effects.refused:1`.
///
/// Always `"urn:ads:event:effects.refused:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventEffectsRefusedV1Type;

impl EventEffectsRefusedV1Type {
    pub const VALUE: &str = "urn:ads:event:effects.refused:1";
}

impl Serialize for EventEffectsRefusedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventEffectsRefusedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_EFFECTS_REFUSED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_EFFECTS_REFUSED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^effects/");

/// `stream` in `urn:ads:event:effects.refused:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventEffectsRefusedV1Stream(String);

impl EventEffectsRefusedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventEffectsRefusedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_EFFECTS_REFUSED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_EFFECTS_REFUSED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventEffectsRefusedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventEffectsRefusedV1Stream> for String {
    fn from(value: EventEffectsRefusedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventEffectsRefusedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventEffectsRefusedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `seq` in `urn:ads:event:effects.refused:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventEffectsRefusedV1Seq;

impl EventEffectsRefusedV1Seq {
    pub const VALUE: i64 = 1;
}

impl Serialize for EventEffectsRefusedV1Seq {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventEffectsRefusedV1Seq {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:effects.refused:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEffectsRefusedV1Data {
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub error_code: EventEffectsRefusedV1DataErrorCode,
}

static EVENT_EFFECTS_REFUSED_V1_DATA_ERROR_CODE_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `data.error_code` in `urn:ads:event:effects.refused:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventEffectsRefusedV1DataErrorCode(String);

impl EventEffectsRefusedV1DataErrorCode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventEffectsRefusedV1DataErrorCode",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_EFFECTS_REFUSED_V1_DATA_ERROR_CODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventEffectsRefusedV1DataErrorCode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventEffectsRefusedV1DataErrorCode> for String {
    fn from(value: EventEffectsRefusedV1DataErrorCode) -> String {
        value.0
    }
}

impl fmt::Display for EventEffectsRefusedV1DataErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventEffectsRefusedV1DataErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
