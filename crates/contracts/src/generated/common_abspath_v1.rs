// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.abspath.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_ABSPATH_V1_PATTERN: Pattern = Pattern::new(
    r#"^([A-Z]:\\([^\\/:"<>|]*[^\\/:"<>|. ](\\[^\\/:"<>|]*[^\\/:"<>|. ])*)?|/([^/\\]+(/[^/\\]+)*)?)$"#,
);
static COMMON_ABSPATH_V1_EXCLUDED_1: Pattern = Pattern::new(r"(^|[\\/])\.\.?([\\/]|$)");
static COMMON_ABSPATH_V1_EXCLUDED_2: Pattern = Pattern::new(r"[$%]");
static COMMON_ABSPATH_V1_EXCLUDED_3: Pattern = Pattern::new(r"(^|[\\/])~");
static COMMON_ABSPATH_V1_EXCLUDED_4: Pattern = Pattern::new(r"[*?]");
static COMMON_ABSPATH_V1_EXCLUDED_5: Pattern =
    Pattern::new(r"[\x{0000}-\x{001f}\x{007f}-\x{009f}]");

/// Caminho absoluto normalizado. Caminho absoluto literal, já normalizado, no estilo Windows (unidade em maiúscula e barra invertida) ou POSIX (barra normal): um separador só, sem separador no fim (salvo a raiz), sem componente '.' ou '..', sem variável ($ ou %), sem til no começo de componente, sem curinga e sem caminho de rede.
///
/// `urn:ads:common:abspath:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonAbspathV1(String);

impl CommonAbspathV1 {
    pub const MAX_CHARS: usize = 4096;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonAbspathV1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
            && COMMON_ABSPATH_V1_PATTERN.matches(text)
            && !COMMON_ABSPATH_V1_EXCLUDED_1.matches(text)
            && !COMMON_ABSPATH_V1_EXCLUDED_2.matches(text)
            && !COMMON_ABSPATH_V1_EXCLUDED_3.matches(text)
            && !COMMON_ABSPATH_V1_EXCLUDED_4.matches(text)
            && !COMMON_ABSPATH_V1_EXCLUDED_5.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonAbspathV1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonAbspathV1> for String {
    fn from(value: CommonAbspathV1) -> String {
        value.0
    }
}

impl fmt::Display for CommonAbspathV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonAbspathV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
