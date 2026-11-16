// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.server-message.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

use super::command_response_v2::CommandResponseV2;
use super::event_envelope_v1::EventEnvelopeV1;

/// Mensagem do núcleo à interface. Tudo o que o núcleo manda à interface pelo WebSocket da API local: a resposta a um comando (command.response:2) ou um evento de um stream assinado (event.envelope:1; a saída do terminal é o evento session.output:1). Cada mensagem é uma das duas, nunca as duas.
///
/// `urn:ads:contract:local-api.server-message:1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiServerMessageV1 {
    Response(CommandResponseV2),
    Envelope(EventEnvelopeV1),
}
