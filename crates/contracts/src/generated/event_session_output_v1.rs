// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.output.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_base64_v1::CommonBase64V1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Literal, Pattern, Rule};

static EVENT_SESSION_OUTPUT_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/data_class",
    Rule::Properties(&[(
        "data_class",
        Rule::Enum(&[Literal::Str("private"), Literal::Str("secret")]),
    )]),
)];

/// Saída da sessão. Bytes que o terminal produziu, em base64, com deslocamento no fluxo da sessão. No transporte ao vivo viajam em quadro binário; este é o registro.
///
/// `urn:ads:event:session.output:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionOutputV1Fields")]
pub struct EventSessionOutputV1(EventSessionOutputV1Fields);

/// The fields of [`EventSessionOutputV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionOutputV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventSessionOutputV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionOutputV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventSessionOutputV1Data,
}

impl EventSessionOutputV1 {
    pub fn new(fields: EventSessionOutputV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionOutputV1",
            &fields,
            EVENT_SESSION_OUTPUT_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionOutputV1Fields {
        self.0
    }
}

impl Deref for EventSessionOutputV1 {
    type Target = EventSessionOutputV1Fields;

    fn deref(&self) -> &EventSessionOutputV1Fields {
        &self.0
    }
}

impl TryFrom<EventSessionOutputV1Fields> for EventSessionOutputV1 {
    type Error = ContractError;

    fn try_from(fields: EventSessionOutputV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionOutputV1> for EventSessionOutputV1Fields {
    fn from(value: EventSessionOutputV1) -> EventSessionOutputV1Fields {
        value.0
    }
}

impl Serialize for EventSessionOutputV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:session.output:1`.
///
/// Always `"urn:ads:event:session.output:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionOutputV1Type;

impl EventSessionOutputV1Type {
    pub const VALUE: &str = "urn:ads:event:session.output:1";
}

impl Serialize for EventSessionOutputV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionOutputV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_OUTPUT_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_OUTPUT_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.output:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionOutputV1Stream(String);

impl EventSessionOutputV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionOutputV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_OUTPUT_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_OUTPUT_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionOutputV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionOutputV1Stream> for String {
    fn from(value: EventSessionOutputV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionOutputV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionOutputV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:session.output:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionOutputV1Data {
    pub session_id: CommonIdV1,
    pub offset: u64,
    pub length: NonZeroU64,
    pub encoding: EventSessionOutputV1DataEncoding,
    pub payload: CommonBase64V1,
}

/// `data.encoding` in `urn:ads:event:session.output:1`.
///
/// Always `"base64"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionOutputV1DataEncoding;

impl EventSessionOutputV1DataEncoding {
    pub const VALUE: &str = "base64";
}

impl Serialize for EventSessionOutputV1DataEncoding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionOutputV1DataEncoding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}
