// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/relay.device.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support::{ContractError, Pattern};

/// Aparelho inscrito no relé. O que o relé guarda de um aparelho: o identificador opaco e a chave pública (32 bytes em base64). Chave privada, passkey e credencial de IA nunca chegam ao relé; a passkey é conferida pelo núcleo, dentro do canal cifrado.
///
/// `urn:ads:relay:device:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayDeviceV1 {
    pub device_id: CommonIdV1,
    pub public_key: RelayDeviceV1PublicKey,
    pub registered_at: CommonTimestampV1,
}

static RELAY_DEVICE_V1_PUBLIC_KEY_PATTERN: Pattern = Pattern::new(r"^[A-Za-z0-9+/]{43}=$");

/// `public_key` in `urn:ads:relay:device:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RelayDeviceV1PublicKey(String);

impl RelayDeviceV1PublicKey {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RelayDeviceV1PublicKey", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RELAY_DEVICE_V1_PUBLIC_KEY_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RelayDeviceV1PublicKey {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RelayDeviceV1PublicKey> for String {
    fn from(value: RelayDeviceV1PublicKey) -> String {
        value.0
    }
}

impl fmt::Display for RelayDeviceV1PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RelayDeviceV1PublicKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
