// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.base64.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_BASE64_V1_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// Texto em base64. Bytes em base64 padrão (RFC 4648) com preenchimento; vazio é permitido.
///
/// `urn:ads:common:base64:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonBase64V1(String);

impl CommonBase64V1 {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonBase64V1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMON_BASE64_V1_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonBase64V1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonBase64V1> for String {
    fn from(value: CommonBase64V1) -> String {
        value.0
    }
}

impl fmt::Display for CommonBase64V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonBase64V1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
