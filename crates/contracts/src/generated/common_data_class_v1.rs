// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.data-class.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

/// Classe do dado. Classe de sensibilidade de um dado: public, internal, private ou secret. Segredo nunca sai da máquina.
///
/// `urn:ads:common:data-class:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommonDataClassV1 {
    #[serde(rename = "public")]
    Public,
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "private")]
    Private,
    #[serde(rename = "secret")]
    Secret,
}

impl CommonDataClassV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Private => "private",
            Self::Secret => "secret",
        }
    }
}
