// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.ended.v1.schema.json. Não edite à mão.

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

/// Sessão encerrada. A sessão terminou: o processo saiu, foi morto, o host de PTY reiniciou ou houve erro. Saída normal traz o código de saída.
///
/// `urn:ads:event:session.ended:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionEndedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionEndedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionEndedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventSessionEndedV1Data,
}

/// `type` in `urn:ads:event:session.ended:1`.
///
/// Always `"urn:ads:event:session.ended:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionEndedV1Type;

impl EventSessionEndedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.ended:1";
}

impl Serialize for EventSessionEndedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionEndedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_ENDED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_ENDED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.ended:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionEndedV1Stream(String);

impl EventSessionEndedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventSessionEndedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_ENDED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_ENDED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionEndedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionEndedV1Stream> for String {
    fn from(value: EventSessionEndedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionEndedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionEndedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static EVENT_SESSION_ENDED_V1_DATA_RULES: &[(&str, Rule)] = &[(
    "#/properties/data/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[("reason", Rule::Const(Literal::Str("exit")))]),
            Rule::Required(&["reason"]),
        ]),
        then: Some(&Rule::Properties(&[(
            "exit_code",
            Rule::Type(&[JsonType::Integer]),
        )])),
        otherwise: None,
    },
)];

/// `data` in `urn:ads:event:session.ended:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventSessionEndedV1DataFields")]
pub struct EventSessionEndedV1Data(EventSessionEndedV1DataFields);

/// The fields of [`EventSessionEndedV1Data`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionEndedV1DataFields {
    pub session_id: CommonIdV1,
    pub reason: EventSessionEndedV1DataReason,
    #[serde(deserialize_with = "support::nullable")]
    pub exit_code: Option<i64>,
}

impl EventSessionEndedV1Data {
    pub fn new(fields: EventSessionEndedV1DataFields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventSessionEndedV1Data",
            &fields,
            EVENT_SESSION_ENDED_V1_DATA_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventSessionEndedV1DataFields {
        self.0
    }
}

impl Deref for EventSessionEndedV1Data {
    type Target = EventSessionEndedV1DataFields;

    fn deref(&self) -> &EventSessionEndedV1DataFields {
        &self.0
    }
}

impl TryFrom<EventSessionEndedV1DataFields> for EventSessionEndedV1Data {
    type Error = ContractError;

    fn try_from(fields: EventSessionEndedV1DataFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventSessionEndedV1Data> for EventSessionEndedV1DataFields {
    fn from(value: EventSessionEndedV1Data) -> EventSessionEndedV1DataFields {
        value.0
    }
}

impl Serialize for EventSessionEndedV1Data {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.reason` in `urn:ads:event:session.ended:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionEndedV1DataReason {
    #[serde(rename = "exit")]
    Exit,
    #[serde(rename = "killed")]
    Killed,
    #[serde(rename = "host_restarted")]
    HostRestarted,
    #[serde(rename = "error")]
    Error,
}

impl EventSessionEndedV1DataReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::Killed => "killed",
            Self::HostRestarted => "host_restarted",
            Self::Error => "error",
        }
    }
}
