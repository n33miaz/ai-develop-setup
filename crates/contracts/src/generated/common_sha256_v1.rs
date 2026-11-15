// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.sha256.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_SHA256_V1_PATTERN: Pattern = Pattern::new(r"^sha256:[0-9a-f]{64}$");

/// Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
///
/// `urn:ads:common:sha256:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonSha256V1(String);

impl CommonSha256V1 {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonSha256V1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMON_SHA256_V1_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonSha256V1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonSha256V1> for String {
    fn from(value: CommonSha256V1) -> String {
        value.0
    }
}

impl fmt::Display for CommonSha256V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonSha256V1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
