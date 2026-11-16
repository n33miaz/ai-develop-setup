// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.abspath.v2.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_ABSPATH_V2_PATTERN: Pattern = Pattern::new(
    r#"^([A-Z]:\\([^\\/:"<>|]*[^\\/:"<>|. ](\\[^\\/:"<>|]*[^\\/:"<>|. ])*)?|/([^/\\]+(/[^/\\]+)*)?)$"#,
);
static COMMON_ABSPATH_V2_EXCLUDED_1: Pattern = Pattern::new(r"(^|[\\/])\.\.?([\\/]|$)");
static COMMON_ABSPATH_V2_EXCLUDED_2: Pattern = Pattern::new(r"[$%]");
static COMMON_ABSPATH_V2_EXCLUDED_3: Pattern = Pattern::new(r"(^|[\\/])~");
static COMMON_ABSPATH_V2_EXCLUDED_4: Pattern = Pattern::new(r"[*?]");
static COMMON_ABSPATH_V2_EXCLUDED_5: Pattern =
    Pattern::new(r"[\x{0000}-\x{001f}\x{007f}-\x{009f}]");
static COMMON_ABSPATH_V2_EXCLUDED_6: Pattern = Pattern::new(
    r"^[A-Z]:\\([^\\]*\\)*([Cc][Oo][Nn]|[Pp][Rr][Nn]|[Aa][Uu][Xx]|[Nn][Uu][Ll]|([Cc][Oo][Mm]|[Ll][Pp][Tt])[0-9\x{00b9}\x{00b2}\x{00b3}])[ \x{00a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}]*(\.[^\\]*)?(\\|$)",
);
static COMMON_ABSPATH_V2_EXCLUDED_7: Pattern =
    Pattern::new(r"^[A-Z]:\\([^\\]*\\)*[^\\]*~[0-9]+(\.|\\|$)");

/// Caminho absoluto normalizado. Caminho absoluto literal, já normalizado, no estilo Windows (unidade em maiúscula e barra invertida) ou POSIX (barra normal): um separador só, sem separador no fim (salvo a raiz), sem componente '.' ou '..', sem variável ($ ou %), sem til no começo de componente, sem curinga e sem caminho de rede. A versão 2 recusa também, no estilo Windows, o que é apelido de outro caminho: componente que é nome de dispositivo (CON, PRN, AUX, NUL, COM0 a COM9, LPT0 a LPT9 e os mesmos com ¹, ² ou ³, com qualquer extensão) e nome curto 8.3 (til seguido de dígitos até o fim do nome ou até um ponto).
///
/// `urn:ads:common:abspath:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonAbspathV2(String);

impl CommonAbspathV2 {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonAbspathV2", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=4096).contains(&length)
            && COMMON_ABSPATH_V2_PATTERN.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_1.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_2.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_3.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_4.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_5.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_6.matches(text)
            && !COMMON_ABSPATH_V2_EXCLUDED_7.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonAbspathV2 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonAbspathV2> for String {
    fn from(value: CommonAbspathV2) -> String {
        value.0
    }
}

impl fmt::Display for CommonAbspathV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonAbspathV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
