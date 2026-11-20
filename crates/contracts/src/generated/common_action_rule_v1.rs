// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/common.action-rule.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::support::{ContractError, Pattern};

static COMMON_ACTION_RULE_V1_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)?$");

/// Regra do portão de ações. Código estável de uma regra do portão de ações que disparou num plano: palavras minúsculas com sublinhado e, quando a regra tem um detalhe, um ponto e o detalhe (protected.user_home). A lista dos códigos em uso está no README; código novo não muda a versão.
///
/// `urn:ads:common:action-rule:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct CommonActionRuleV1(String);

impl CommonActionRuleV1 {
    pub const MAX_CHARS: usize = 64;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("CommonActionRuleV1", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= Self::MAX_CHARS && COMMON_ACTION_RULE_V1_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommonActionRuleV1 {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<CommonActionRuleV1> for String {
    fn from(value: CommonActionRuleV1) -> String {
        value.0
    }
}

impl fmt::Display for CommonActionRuleV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CommonActionRuleV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
