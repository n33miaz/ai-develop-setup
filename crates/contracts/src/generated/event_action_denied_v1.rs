// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.action.denied.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_action_rule_v1::CommonActionRuleV1;
use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Ação negada. O portão negou o plano: nada será executado e não cabe aprovação. É o único evento da ação (vem no lugar de action.planned) e cita o hash do plano negado e as regras que o negaram; os caminhos ficam no registro do plano.
///
/// `urn:ads:event:action.denied:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventActionDeniedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventActionDeniedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventActionDeniedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventActionDeniedV1Data,
}

/// `type` in `urn:ads:event:action.denied:1`.
///
/// Always `"urn:ads:event:action.denied:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventActionDeniedV1Type;

impl EventActionDeniedV1Type {
    pub const VALUE: &str = "urn:ads:event:action.denied:1";
}

impl Serialize for EventActionDeniedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventActionDeniedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ACTION_DENIED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ACTION_DENIED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^action/");

/// `stream` in `urn:ads:event:action.denied:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventActionDeniedV1Stream(String);

impl EventActionDeniedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventActionDeniedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ACTION_DENIED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ACTION_DENIED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventActionDeniedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventActionDeniedV1Stream> for String {
    fn from(value: EventActionDeniedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventActionDeniedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventActionDeniedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:action.denied:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventActionDeniedV1Data {
    pub action_id: CommonIdV1,
    pub verb: EventActionDeniedV1DataVerb,
    pub plan_hash: CommonSha256V1,
    pub rules: EventActionDeniedV1DataRules,
}

static EVENT_ACTION_DENIED_V1_DATA_VERB_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$");

/// `data.verb` in `urn:ads:event:action.denied:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventActionDeniedV1DataVerb(String);

impl EventActionDeniedV1DataVerb {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventActionDeniedV1DataVerb", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 64 && EVENT_ACTION_DENIED_V1_DATA_VERB_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventActionDeniedV1DataVerb {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventActionDeniedV1DataVerb> for String {
    fn from(value: EventActionDeniedV1DataVerb) -> String {
        value.0
    }
}

impl fmt::Display for EventActionDeniedV1DataVerb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventActionDeniedV1DataVerb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data.rules` in `urn:ads:event:action.denied:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonActionRuleV1>")]
pub struct EventActionDeniedV1DataRules(Vec<CommonActionRuleV1>);

impl EventActionDeniedV1DataRules {
    pub fn new(items: Vec<CommonActionRuleV1>) -> Result<Self, ContractError> {
        if (1..=100).contains(&items.len()) && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "EventActionDeniedV1DataRules",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommonActionRuleV1] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommonActionRuleV1> {
        self.0
    }
}

impl TryFrom<Vec<CommonActionRuleV1>> for EventActionDeniedV1DataRules {
    type Error = ContractError;

    fn try_from(items: Vec<CommonActionRuleV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<EventActionDeniedV1DataRules> for Vec<CommonActionRuleV1> {
    fn from(value: EventActionDeniedV1DataRules) -> Vec<CommonActionRuleV1> {
        value.0
    }
}

impl Serialize for EventActionDeniedV1DataRules {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
