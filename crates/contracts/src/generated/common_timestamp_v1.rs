// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.timestamp.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support;
use crate::support::{ContractError, Pattern};

static COMMON_TIMESTAMP_V1_PATTERN: Pattern =
    Pattern::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]{1,6})?Z$");

/// Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
///
/// `urn:ads:common:timestamp:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonTimestampV1(String);

impl CommonTimestampV1 {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonTimestampV1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMON_TIMESTAMP_V1_PATTERN.matches(text) && support::is_date_time(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonTimestampV1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonTimestampV1> for String {
    fn from(value: CommonTimestampV1) -> String {
        value.0
    }
}

impl fmt::Display for CommonTimestampV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonTimestampV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
