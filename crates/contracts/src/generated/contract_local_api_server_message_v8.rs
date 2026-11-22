// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.server-message.v8.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

use super::command_approval_list_response_v3::CommandApprovalListResponseV3;
use super::command_harness_list_response_v2::CommandHarnessListResponseV2;
use super::command_queue_get_response_v2::CommandQueueGetResponseV2;
use super::command_response_v2::CommandResponseV2;
use super::event_envelope_v1::EventEnvelopeV1;

/// Mensagem do núcleo à interface. Mensagens do núcleo à interface pelo WebSocket da API local. A versão 8 passa a usar a resposta de harness.list que informa os apelidos das pastas ausentes na partida.
///
/// `urn:ads:contract:local-api.server-message:8`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiServerMessageV8 {
    Response(CommandResponseV2),
    HarnessListResponse(CommandHarnessListResponseV2),
    QueueGetResponse(CommandQueueGetResponseV2),
    ApprovalListResponse(CommandApprovalListResponseV3),
    Envelope(EventEnvelopeV1),
}
