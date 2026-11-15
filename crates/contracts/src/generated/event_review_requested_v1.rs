// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.review.requested.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Revisão pedida. Um revisor foi chamado para avaliar a entrega de uma tarefa. Só o digest do diff vai no evento; o diff fica fora.
///
/// `urn:ads:event:review.requested:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventReviewRequestedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventReviewRequestedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventReviewRequestedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventReviewRequestedV1Data,
}

/// `type` in `urn:ads:event:review.requested:1`.
///
/// Always `"urn:ads:event:review.requested:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventReviewRequestedV1Type;

impl EventReviewRequestedV1Type {
    pub const VALUE: &str = "urn:ads:event:review.requested:1";
}

impl Serialize for EventReviewRequestedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventReviewRequestedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_REVIEW_REQUESTED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_REVIEW_REQUESTED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^review/");

/// `stream` in `urn:ads:event:review.requested:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventReviewRequestedV1Stream(String);

impl EventReviewRequestedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventReviewRequestedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_REVIEW_REQUESTED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_REVIEW_REQUESTED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventReviewRequestedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventReviewRequestedV1Stream> for String {
    fn from(value: EventReviewRequestedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventReviewRequestedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventReviewRequestedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:review.requested:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventReviewRequestedV1Data {
    pub review_id: CommonIdV1,
    pub task_id: CommonIdV1,
    pub diff_sha: CommonSha256V1,
}
