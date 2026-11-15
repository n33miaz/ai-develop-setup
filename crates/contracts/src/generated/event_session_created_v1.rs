// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.created.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_SESSION_CREATED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[(
                "data",
                Rule::AllOf(&[
                    Rule::Type(&[JsonType::Object]),
                    Rule::Required(&["display_name"]),
                ]),
            )]),
            Rule::Required(&["data"]),
        ]),
        then: Some(&Rule::Properties(&[(
            "data_class",
            Rule::Enum(&[
                Literal::Str("internal"),
                Literal::Str("private"),
                Literal::Str("secret"),
            ]),
        )])),
        otherwise: None,
    },
)];

/// Sessão criada. Uma sessão de terminal foi aberta no host de PTY para um agente de IA. O nome de exibição, opcional, é o rótulo que a interface mostra.
///
/// `urn:ads:event:session.created:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionCreatedV1Fields")]
pub struct EventSessionCreatedV1(EventSessionCreatedV1Fields);

/// The fields of [`EventSessionCreatedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionCreatedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventSessionCreatedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionCreatedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventSessionCreatedV1Data,
}

impl EventSessionCreatedV1 {
    pub fn new(fields: EventSessionCreatedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionCreatedV1",
            &fields,
            EVENT_SESSION_CREATED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionCreatedV1Fields {
        self.0
    }
}

impl Deref for EventSessionCreatedV1 {
    type Target = EventSessionCreatedV1Fields;

    fn deref(&self) -> &EventSessionCreatedV1Fields {
        &self.0
    }
}

impl TryFrom<EventSessionCreatedV1Fields> for EventSessionCreatedV1 {
    type Error = ContractError;

    fn try_from(fields: EventSessionCreatedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionCreatedV1> for EventSessionCreatedV1Fields {
    fn from(value: EventSessionCreatedV1) -> EventSessionCreatedV1Fields {
        value.0
    }
}

impl Serialize for EventSessionCreatedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:session.created:1`.
///
/// Always `"urn:ads:event:session.created:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionCreatedV1Type;

impl EventSessionCreatedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.created:1";
}

impl Serialize for EventSessionCreatedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionCreatedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_CREATED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_CREATED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionCreatedV1Stream(String);

impl EventSessionCreatedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionCreatedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_CREATED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_CREATED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionCreatedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionCreatedV1Stream> for String {
    fn from(value: EventSessionCreatedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionCreatedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionCreatedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:session.created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionCreatedV1Data {
    pub session_id: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub task_id: Option<CommonIdV1>,
    pub harness: CommonIdV1,
    pub mode: EventSessionCreatedV1DataMode,
    pub cols: EventSessionCreatedV1DataCols,
    pub rows: EventSessionCreatedV1DataRows,
    pub workdir_alias: CommonIdV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub display_name: Option<EventSessionCreatedV1DataDisplayName>,
}

/// `data.mode` in `urn:ads:event:session.created:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionCreatedV1DataMode {
    #[serde(rename = "interactive")]
    Interactive,
    #[serde(rename = "headless")]
    Headless,
}

impl EventSessionCreatedV1DataMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Headless => "headless",
        }
    }
}

/// `data.cols` in `urn:ads:event:session.created:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionCreatedV1DataCols(u16);

impl EventSessionCreatedV1DataCols {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionCreatedV1DataCols",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionCreatedV1DataCols {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionCreatedV1DataCols> for u16 {
    fn from(value: EventSessionCreatedV1DataCols) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionCreatedV1DataCols {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.rows` in `urn:ads:event:session.created:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventSessionCreatedV1DataRows(u16);

impl EventSessionCreatedV1DataRows {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=1000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventSessionCreatedV1DataRows",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventSessionCreatedV1DataRows {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventSessionCreatedV1DataRows> for u16 {
    fn from(value: EventSessionCreatedV1DataRows) -> u16 {
        value.0
    }
}

impl Serialize for EventSessionCreatedV1DataRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static EVENT_SESSION_CREATED_V1_DATA_DISPLAY_NAME_PATTERN: Pattern =
    Pattern::new(r"^[^\x{0000}-\x{001f}\x{007f}-\x{009f}]*$");

/// Nome de exibição da sessão, numa linha.
///
/// `data.display_name` in `urn:ads:event:session.created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionCreatedV1DataDisplayName(String);

impl EventSessionCreatedV1DataDisplayName {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionCreatedV1DataDisplayName",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=80).contains(&length)
            && EVENT_SESSION_CREATED_V1_DATA_DISPLAY_NAME_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionCreatedV1DataDisplayName {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionCreatedV1DataDisplayName> for String {
    fn from(value: EventSessionCreatedV1DataDisplayName) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionCreatedV1DataDisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionCreatedV1DataDisplayName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
