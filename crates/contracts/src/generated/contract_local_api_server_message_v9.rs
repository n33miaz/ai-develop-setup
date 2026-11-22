// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.server-message.v9.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

use super::command_approval_list_response_v4::CommandApprovalListResponseV4;
use super::command_harness_list_response_v2::CommandHarnessListResponseV2;
use super::command_queue_get_response_v2::CommandQueueGetResponseV2;
use super::command_response_v2::CommandResponseV2;
use super::event_envelope_v1::EventEnvelopeV1;

/// Mensagem do núcleo à interface. Mensagens do núcleo à interface pelo WebSocket da API local. A versão 9 inclui a resposta de approval.list que informa quando a política vigente impede a aprovação.
///
/// `urn:ads:contract:local-api.server-message:9`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiServerMessageV9 {
    Response(CommandResponseV2),
    HarnessListResponse(CommandHarnessListResponseV2),
    QueueGetResponse(CommandQueueGetResponseV2),
    ApprovalListResponse(CommandApprovalListResponseV4),
    Envelope(EventEnvelopeV1),
}
