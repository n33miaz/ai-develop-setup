// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.workdir-alias.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_WORKDIR_ALIAS_V1_PATTERN: Pattern =
    Pattern::new(r"^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9_-])?$");
static COMMON_WORKDIR_ALIAS_V1_EXCLUDED: Pattern =
    Pattern::new(r"(^((con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|$)))|\.\.");

/// Apelido de pasta de trabalho. Nome cadastrado de uma pasta de trabalho, em vez do caminho: minúsculas, dígitos e . _ -, de 1 a 64 caracteres, sem '..', sem terminar em ponto e sem nome de dispositivo do Windows.
///
/// `urn:ads:common:workdir-alias:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonWorkdirAliasV1(String);

impl CommonWorkdirAliasV1 {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonWorkdirAliasV1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        COMMON_WORKDIR_ALIAS_V1_PATTERN.matches(text)
            && !COMMON_WORKDIR_ALIAS_V1_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonWorkdirAliasV1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonWorkdirAliasV1> for String {
    fn from(value: CommonWorkdirAliasV1) -> String {
        value.0
    }
}

impl fmt::Display for CommonWorkdirAliasV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonWorkdirAliasV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
