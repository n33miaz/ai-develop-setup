// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.account.status_observed.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_abspath_v2::CommonAbspathV2;
use super::common_actor_v1::CommonActorV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Status de conta observado. Metadados permitidos do status de um perfil. A pasta é um dado privado.
///
/// `urn:ads:event:account.status_observed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAccountStatusObservedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventAccountStatusObservedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventAccountStatusObservedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: EventAccountStatusObservedV1DataClass,
    pub data: EventAccountStatusObservedV1Data,
}

/// `type` in `urn:ads:event:account.status_observed:1`.
///
/// Always `"urn:ads:event:account.status_observed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventAccountStatusObservedV1Type;

impl EventAccountStatusObservedV1Type {
    pub const VALUE: &str = "urn:ads:event:account.status_observed:1";
}

impl Serialize for EventAccountStatusObservedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventAccountStatusObservedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ACCOUNT_STATUS_OBSERVED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ACCOUNT_STATUS_OBSERVED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^account/");

/// `stream` in `urn:ads:event:account.status_observed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventAccountStatusObservedV1Stream(String);

impl EventAccountStatusObservedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "EventAccountStatusObservedV1Stream",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ACCOUNT_STATUS_OBSERVED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ACCOUNT_STATUS_OBSERVED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventAccountStatusObservedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventAccountStatusObservedV1Stream> for String {
    fn from(value: EventAccountStatusObservedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventAccountStatusObservedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventAccountStatusObservedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data_class` in `urn:ads:event:account.status_observed:1`.
///
/// Always `"private"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventAccountStatusObservedV1DataClass;

impl EventAccountStatusObservedV1DataClass {
    pub const VALUE: &str = "private";
}

impl Serialize for EventAccountStatusObservedV1DataClass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventAccountStatusObservedV1DataClass {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `data` in `urn:ads:event:account.status_observed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAccountStatusObservedV1Data {
    pub harness: CommonIdV1,
    pub account_alias: CommonIdV1,
    pub logged_in: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub auth_method: Option<CommonIdV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub provider: Option<CommonIdV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub plan_type: Option<CommonIdV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub config_directory: Option<CommonAbspathV2>,
}
