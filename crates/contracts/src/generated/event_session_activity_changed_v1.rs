// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.session.activity_changed.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Atividade da sessão mudou. A sessão passou de ativa para ociosa ou de ociosa para ativa. Ativa: o terminal produziu saída ou recebeu entrada há pouco; ociosa: está parada, à espera. Sessão recém-criada conta como ativa até o primeiro evento destes; o tempo de silêncio que a torna ociosa é parâmetro de quem emite.
///
/// `urn:ads:event:session.activity_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionActivityChangedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventSessionActivityChangedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventSessionActivityChangedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventSessionActivityChangedV1Data,
}

/// `type` in `urn:ads:event:session.activity_changed:1`.
///
/// Always `"urn:ads:event:session.activity_changed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventSessionActivityChangedV1Type;

impl EventSessionActivityChangedV1Type {
    pub const VALUE: &str = "urn:ads:event:session.activity_changed:1";
}

impl Serialize for EventSessionActivityChangedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventSessionActivityChangedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_SESSION_ACTIVITY_CHANGED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_SESSION_ACTIVITY_CHANGED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^session/");

/// `stream` in `urn:ads:event:session.activity_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventSessionActivityChangedV1Stream(String);

impl EventSessionActivityChangedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventSessionActivityChangedV1Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_SESSION_ACTIVITY_CHANGED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_SESSION_ACTIVITY_CHANGED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventSessionActivityChangedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventSessionActivityChangedV1Stream> for String {
    fn from(value: EventSessionActivityChangedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventSessionActivityChangedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventSessionActivityChangedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:session.activity_changed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSessionActivityChangedV1Data {
    pub session_id: CommonIdV1,
    pub activity: EventSessionActivityChangedV1DataActivity,
}

/// `data.activity` in `urn:ads:event:session.activity_changed:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventSessionActivityChangedV1DataActivity {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "idle")]
    Idle,
}

impl EventSessionActivityChangedV1DataActivity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
        }
    }
}
