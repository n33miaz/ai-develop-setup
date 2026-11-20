// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.queue.policy_changed.v1.schema.json. Não edite à mão.

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

static EVENT_QUEUE_POLICY_CHANGED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Política da fila alterada. Registra a política completa aprovada pelo dono para as admissões posteriores no stream.
///
/// `urn:ads:event:queue.policy_changed:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventQueuePolicyChangedV1Fields")]
pub struct EventQueuePolicyChangedV1(EventQueuePolicyChangedV1Fields);

/// The fields of [`EventQueuePolicyChangedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueuePolicyChangedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventQueuePolicyChangedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventQueuePolicyChangedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventQueuePolicyChangedV1Data,
}

impl EventQueuePolicyChangedV1 {
    pub fn new(fields: EventQueuePolicyChangedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventQueuePolicyChangedV1",
            &fields,
            EVENT_QUEUE_POLICY_CHANGED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventQueuePolicyChangedV1Fields {
        self.0
    }
}

impl Deref for EventQueuePolicyChangedV1 {
    type Target = EventQueuePolicyChangedV1Fields;

    fn deref(&self) -> &EventQueuePolicyChangedV1Fields {
        &self.0
    }
}

impl TryFrom<EventQueuePolicyChangedV1Fields> for EventQueuePolicyChangedV1 {
    type Error = ContractError;

    fn try_from(fields: EventQueuePolicyChangedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventQueuePolicyChangedV1> for EventQueuePolicyChangedV1Fields {
    fn from(value: EventQueuePolicyChangedV1) -> EventQueuePolicyChangedV1Fields {
        value.0
    }
}

impl Serialize for EventQueuePolicyChangedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:queue.policy_changed:1`.
///
/// Always `"urn:ads:event:queue.policy_changed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventQueuePolicyChangedV1Type;

impl EventQueuePolicyChangedV1Type {
    pub const VALUE: &str = "urn:ads:event:queue.policy_changed:1";
}

impl Serialize for EventQueuePolicyChangedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventQueuePolicyChangedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_QUEUE_POLICY_CHANGED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_QUEUE_POLICY_CHANGED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^queue/");

/// `stream` in `urn:ads:event:queue.policy_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventQueuePolicyChangedV1Stream(String);

impl EventQueuePolicyChangedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventQueuePolicyChangedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_QUEUE_POLICY_CHANGED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_QUEUE_POLICY_CHANGED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventQueuePolicyChangedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventQueuePolicyChangedV1Stream> for String {
    fn from(value: EventQueuePolicyChangedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventQueuePolicyChangedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventQueuePolicyChangedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:queue.policy_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueuePolicyChangedV1Data {
    pub queue_id: CommonIdV1,
    pub owner_id: CommonIdV1,
    pub via: EventQueuePolicyChangedV1DataVia,
    pub policy: EventQueuePolicyChangedV1DataPolicy,
}

/// `data.via` in `urn:ads:event:queue.policy_changed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventQueuePolicyChangedV1DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl EventQueuePolicyChangedV1DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}

/// `data.policy` in `urn:ads:event:queue.policy_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQueuePolicyChangedV1DataPolicy {
    pub attempt_limit: EventQueuePolicyChangedV1DataPolicyAttemptLimit,
    pub admission_limit: NonZeroU64,
    pub window_seconds: NonZeroU64,
    pub window_admissions: NonZeroU64,
    pub manual_window_admissions: NonZeroU64,
    pub bug_rule_window_admissions: NonZeroU64,
    pub clock_skew_seconds: u64,
    pub clock_rewind_seconds: u64,
    pub host_confirm_seconds: NonZeroU64,
}

/// `data.policy.attempt_limit` in `urn:ads:event:queue.policy_changed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct EventQueuePolicyChangedV1DataPolicyAttemptLimit(u16);

impl EventQueuePolicyChangedV1DataPolicyAttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "EventQueuePolicyChangedV1DataPolicyAttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for EventQueuePolicyChangedV1DataPolicyAttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<EventQueuePolicyChangedV1DataPolicyAttemptLimit> for u16 {
    fn from(value: EventQueuePolicyChangedV1DataPolicyAttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for EventQueuePolicyChangedV1DataPolicyAttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
