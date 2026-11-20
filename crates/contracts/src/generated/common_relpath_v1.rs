// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.relpath.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_RELPATH_V1_PATTERN: Pattern = Pattern::new(r"^[A-Za-z0-9._-][A-Za-z0-9._/-]*$");
static COMMON_RELPATH_V1_EXCLUDED: Pattern = Pattern::new(r"(^|/)\.\.(/|$)");

/// Caminho relativo. Caminho relativo com barra normal, sem raiz, sem letra de unidade e sem segmento '..'.
///
/// `urn:ads:common:relpath:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonRelpathV1(String);

impl CommonRelpathV1 {
    pub const MAX_CHARS: usize = 260;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonRelpathV1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
            && COMMON_RELPATH_V1_PATTERN.matches(text)
            && !COMMON_RELPATH_V1_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonRelpathV1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonRelpathV1> for String {
    fn from(value: CommonRelpathV1) -> String {
        value.0
    }
}

impl fmt::Display for CommonRelpathV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonRelpathV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
