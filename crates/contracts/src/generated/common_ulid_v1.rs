// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.ulid.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_ULID_V1_PATTERN: Pattern = Pattern::new(r"^[0-7][0-9A-HJKMNP-TV-Z]{25}$");

/// Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
///
/// `urn:ads:common:ulid:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonUlidV1(String);

impl CommonUlidV1 {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonUlidV1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMON_ULID_V1_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonUlidV1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonUlidV1> for String {
    fn from(value: CommonUlidV1) -> String {
        value.0
    }
}

impl fmt::Display for CommonUlidV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonUlidV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
