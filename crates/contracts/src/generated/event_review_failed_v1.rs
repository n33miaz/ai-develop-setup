// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.review.failed.v1.schema.json. Não edite à mão.

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

/// Revisão falhou. A revisão terminou sem veredito, com um código de erro estável e a indicação de se vale tentar de novo. Revisão que falha não aprova nada.
///
/// `urn:ads:event:review.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventReviewFailedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventReviewFailedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventReviewFailedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventReviewFailedV1Data,
}

/// `type` in `urn:ads:event:review.failed:1`.
///
/// Always `"urn:ads:event:review.failed:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventReviewFailedV1Type;

impl EventReviewFailedV1Type {
    pub const VALUE: &str = "urn:ads:event:review.failed:1";
}

impl Serialize for EventReviewFailedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventReviewFailedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_REVIEW_FAILED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_REVIEW_FAILED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^review/");

/// `stream` in `urn:ads:event:review.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventReviewFailedV1Stream(String);

impl EventReviewFailedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventReviewFailedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_REVIEW_FAILED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_REVIEW_FAILED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventReviewFailedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventReviewFailedV1Stream> for String {
    fn from(value: EventReviewFailedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventReviewFailedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventReviewFailedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:review.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventReviewFailedV1Data {
    pub review_id: CommonIdV1,
    pub task_id: CommonIdV1,
    pub error_code: EventReviewFailedV1DataErrorCode,
    pub retriable: bool,
}

static EVENT_REVIEW_FAILED_V1_DATA_ERROR_CODE_PATTERN: Pattern =
    Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `data.error_code` in `urn:ads:event:review.failed:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventReviewFailedV1DataErrorCode(String);

impl EventReviewFailedV1DataErrorCode {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventReviewFailedV1DataErrorCode", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_REVIEW_FAILED_V1_DATA_ERROR_CODE_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventReviewFailedV1DataErrorCode {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventReviewFailedV1DataErrorCode> for String {
    fn from(value: EventReviewFailedV1DataErrorCode) -> String {
        value.0
    }
}

impl fmt::Display for EventReviewFailedV1DataErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventReviewFailedV1DataErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
