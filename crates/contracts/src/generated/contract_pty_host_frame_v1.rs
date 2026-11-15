// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.pty-host.frame.v1.schema.json. Não edite à mão.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use super::common_id_v1::CommonIdV1;

/// Cabeçalho do quadro de saída. Cabeçalho, em JSON, do quadro binário que leva os bytes de saída do terminal no contrato pty-host/1: de qual sessão são, o número de sequência do pedaço e quantos bytes vêm logo depois do cabeçalho. Os bytes não fazem parte do JSON; o layout do quadro está no README.
///
/// `urn:ads:contract:pty-host.frame:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostFrameV1 {
    pub session_id: CommonIdV1,
    pub seq: NonZeroU64,
    pub length: NonZeroU64,
}
