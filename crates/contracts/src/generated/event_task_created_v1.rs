// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.task.created.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_TASK_CREATED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[(
                "data",
                Rule::AllOf(&[Rule::Type(&[JsonType::Object]), Rule::Required(&["title"])]),
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

/// Tarefa criada. Uma tarefa nasceu. O objetivo vai só como hash (o texto fica no pacote da tarefa); o título, opcional, é o rótulo curto que a interface mostra.
///
/// `urn:ads:event:task.created:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventTaskCreatedV1Fields")]
pub struct EventTaskCreatedV1(EventTaskCreatedV1Fields);

/// The fields of [`EventTaskCreatedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTaskCreatedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventTaskCreatedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventTaskCreatedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventTaskCreatedV1Data,
}

impl EventTaskCreatedV1 {
    pub fn new(fields: EventTaskCreatedV1Fields) -> Result<Self, ContractError> {
        support::check_rules("EventTaskCreatedV1", &fields, EVENT_TASK_CREATED_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventTaskCreatedV1Fields {
        self.0
    }
}

impl Deref for EventTaskCreatedV1 {
    type Target = EventTaskCreatedV1Fields;

    fn deref(&self) -> &EventTaskCreatedV1Fields {
        &self.0
    }
}

impl TryFrom<EventTaskCreatedV1Fields> for EventTaskCreatedV1 {
    type Error = ContractError;

    fn try_from(fields: EventTaskCreatedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventTaskCreatedV1> for EventTaskCreatedV1Fields {
    fn from(value: EventTaskCreatedV1) -> EventTaskCreatedV1Fields {
        value.0
    }
}

impl Serialize for EventTaskCreatedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:task.created:1`.
///
/// Always `"urn:ads:event:task.created:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventTaskCreatedV1Type;

impl EventTaskCreatedV1Type {
    pub const VALUE: &str = "urn:ads:event:task.created:1";
}

impl Serialize for EventTaskCreatedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventTaskCreatedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_TASK_CREATED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_TASK_CREATED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^task/");

/// `stream` in `urn:ads:event:task.created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventTaskCreatedV1Stream(String);

impl EventTaskCreatedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventTaskCreatedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_TASK_CREATED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_TASK_CREATED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventTaskCreatedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventTaskCreatedV1Stream> for String {
    fn from(value: EventTaskCreatedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventTaskCreatedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventTaskCreatedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:task.created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTaskCreatedV1Data {
    pub task_id: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub parent_task_id: Option<CommonIdV1>,
    pub project_id: CommonIdV1,
    pub objective_hash: CommonSha256V1,
    pub kind: CommonIdV1,
    pub risk: CommonRiskV1,
    pub oracle_declared: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub title: Option<EventTaskCreatedV1DataTitle>,
}

static EVENT_TASK_CREATED_V1_DATA_TITLE_PATTERN: Pattern =
    Pattern::new(r"^[^\x{0000}-\x{001f}\x{007f}-\x{009f}]*$");

/// Título curto e legível da tarefa, numa linha.
///
/// `data.title` in `urn:ads:event:task.created:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventTaskCreatedV1DataTitle(String);

impl EventTaskCreatedV1DataTitle {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventTaskCreatedV1DataTitle", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=120).contains(&length) && EVENT_TASK_CREATED_V1_DATA_TITLE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventTaskCreatedV1DataTitle {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventTaskCreatedV1DataTitle> for String {
    fn from(value: EventTaskCreatedV1DataTitle) -> String {
        value.0
    }
}

impl fmt::Display for EventTaskCreatedV1DataTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventTaskCreatedV1DataTitle {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
