// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.permission_asked.v1.schema.json. Não edite à mão.

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
use crate::support::{ContractError, Literal, Pattern, Rule};

static EVENT_SESSION_PERMISSION_ASKED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/data_class",
    Rule::Properties(&[(
        "data_class",
        Rule::Enum(&[Literal::Str("private"), Literal::Str("secret")]),
    )]),
)];

/// Pergunta de permissão. O CLI de IA pediu permissão para algo (comando, escrita, rede). O resumo pode conter texto do usuário, então a classe é private ou secret.
///
/// `urn:ads:event:session.permission_asked:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionPermissionAskedV1Fields")]
pub struct EventSessionPermissionAskedV1(EventSessionPermissionAskedV1Fields);

/// The fields of [`EventSessionPermissionAskedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionPermissionAskedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventSessionPermissionAskedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionPermissionAskedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventSessionPermissionAskedV1Data,
}

impl EventSessionPermissionAskedV1 {
    pub fn new(fields: EventSessionPermissionAskedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionPermissionAskedV1",
            &fields,
            EVENT_SESSION_PERMISSION_ASKED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionPermissionAskedV1Fields {
        self.0
    }
}

impl Deref for EventSessionPermissionAskedV1 {
    type Target = EventSessionPermissionAskedV1Fields;

    fn deref(&self) -> &EventSessionPermissionAskedV1Fields {
        &self.0
    }
}

impl TryFrom<EventSessionPermissionAskedV1Fields> for EventSessionPermissionAskedV1 {
    type Error = ContractError;

    fn try_from(fields: EventSessionPermissionAskedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionPermissionAskedV1> for EventSessionPermissionAskedV1Fields {
    fn from(value: EventSessionPermissionAskedV1) -> EventSessionPermissionAskedV1Fields {
        value.0
    }
}

impl Serialize for EventSessionPermissionAskedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:session.permission_asked:1`.
///
/// Always `"urn:ads:event:session.permission_asked:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionPermissionAskedV1Type;

impl EventSessionPermissionAskedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.permission_asked:1";
}

impl Serialize for EventSessionPermissionAskedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionPermissionAskedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_PERMISSION_ASKED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_PERMISSION_ASKED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.permission_asked:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionPermissionAskedV1Stream(String);

impl EventSessionPermissionAskedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionPermissionAskedV1Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_PERMISSION_ASKED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_PERMISSION_ASKED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionPermissionAskedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionPermissionAskedV1Stream> for String {
    fn from(value: EventSessionPermissionAskedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionPermissionAskedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionPermissionAskedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:session.permission_asked:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionPermissionAskedV1Data {
    pub session_id: CommonIdV1,
    pub request_id: CommonIdV1,
    pub kind: EventSessionPermissionAskedV1DataKind,
    pub summary: EventSessionPermissionAskedV1DataSummary,
    #[serde(deserialize_with = "support::nullable")]
    pub expires_at: Option<CommonTimestampV1>,
}

/// `data.kind` in `urn:ads:event:session.permission_asked:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionPermissionAskedV1DataKind {
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(rename = "file_write")]
    FileWrite,
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "network")]
    Network,
    #[serde(rename = "other")]
    Other,
}

impl EventSessionPermissionAskedV1DataKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolUse => "tool_use",
            Self::FileWrite => "file_write",
            Self::Command => "command",
            Self::Network => "network",
            Self::Other => "other",
        }
    }
}

/// `data.summary` in `urn:ads:event:session.permission_asked:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionPermissionAskedV1DataSummary(String);

impl EventSessionPermissionAskedV1DataSummary {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionPermissionAskedV1DataSummary",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=500).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionPermissionAskedV1DataSummary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionPermissionAskedV1DataSummary> for String {
    fn from(value: EventSessionPermissionAskedV1DataSummary) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionPermissionAskedV1DataSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionPermissionAskedV1DataSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
