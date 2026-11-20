// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.account.switched.v1.schema.json. Não edite à mão.

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

/// Perfil de conta selecionado. O perfil selecionado para a próxima sessão mudou.
///
/// `urn:ads:event:account.switched:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAccountSwitchedV1 {
    pub id: CommonUlidV1,
    pub r#type: EventAccountSwitchedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventAccountSwitchedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventAccountSwitchedV1Data,
}

/// `type` in `urn:ads:event:account.switched:1`.
///
/// Always `"urn:ads:event:account.switched:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventAccountSwitchedV1Type;

impl EventAccountSwitchedV1Type {
    pub const VALUE: &str = "urn:ads:event:account.switched:1";
}

impl Serialize for EventAccountSwitchedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventAccountSwitchedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_ACCOUNT_SWITCHED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_ACCOUNT_SWITCHED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^account/");

/// `stream` in `urn:ads:event:account.switched:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventAccountSwitchedV1Stream(String);

impl EventAccountSwitchedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventAccountSwitchedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_ACCOUNT_SWITCHED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_ACCOUNT_SWITCHED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventAccountSwitchedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventAccountSwitchedV1Stream> for String {
    fn from(value: EventAccountSwitchedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventAccountSwitchedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventAccountSwitchedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:account.switched:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventAccountSwitchedV1Data {
    pub harness: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub from_account_alias: Option<CommonIdV1>,
    pub account_alias: CommonIdV1,
    pub mode: EventAccountSwitchedV1DataMode,
}

/// `data.mode` in `urn:ads:event:account.switched:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventAccountSwitchedV1DataMode {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "propose")]
    Propose,
    #[serde(rename = "auto")]
    Auto,
}

impl EventAccountSwitchedV1DataMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Propose => "propose",
            Self::Auto => "auto",
        }
    }
}
