// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/relay.envelope.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support::{ContractError, Pattern};

/// Mensagem cifrada do relé. A única coisa que o relé transporta e guarda na caixa de mensagens: texto cifrado de ponta a ponta, de um aparelho para outro, com validade. O relé nunca vê texto em claro nem credencial; depois da validade a mensagem é descartada.
///
/// `urn:ads:relay:envelope:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayEnvelopeV1 {
    pub id: CommonUlidV1,
    pub from: CommonIdV1,
    pub to: CommonIdV1,
    pub expires_at: CommonTimestampV1,
    pub ciphertext: RelayEnvelopeV1Ciphertext,
}

static RELAY_ENVELOPE_V1_CIPHERTEXT_PATTERN: Pattern =
    Pattern::new(r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$");

/// `ciphertext` in `urn:ads:relay:envelope:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RelayEnvelopeV1Ciphertext(String);

impl RelayEnvelopeV1Ciphertext {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RelayEnvelopeV1Ciphertext", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length >= 4 && RELAY_ENVELOPE_V1_CIPHERTEXT_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RelayEnvelopeV1Ciphertext {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RelayEnvelopeV1Ciphertext> for String {
    fn from(value: RelayEnvelopeV1Ciphertext) -> String {
        value.0
    }
}

impl fmt::Display for RelayEnvelopeV1Ciphertext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RelayEnvelopeV1Ciphertext {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
