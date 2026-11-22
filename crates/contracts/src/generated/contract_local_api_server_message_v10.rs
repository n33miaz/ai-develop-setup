// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.server-message.v10.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

use super::command_approval_list_response_v4::CommandApprovalListResponseV4;
use super::command_harness_list_response_v2::CommandHarnessListResponseV2;
use super::command_queue_configured_response_v1::CommandQueueConfiguredResponseV1;
use super::command_queue_get_response_v2::CommandQueueGetResponseV2;
use super::command_response_v2::CommandResponseV2;
use super::event_envelope_v1::EventEnvelopeV1;

/// Mensagem do núcleo à interface. Mensagens do núcleo à interface pelo WebSocket da API local. A versão 10 acrescenta à :9 a resposta de queue.configured.
///
/// `urn:ads:contract:local-api.server-message:10`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiServerMessageV10 {
    Response(CommandResponseV2),
    HarnessListResponse(CommandHarnessListResponseV2),
    QueueGetResponse(CommandQueueGetResponseV2),
    QueueConfiguredResponse(CommandQueueConfiguredResponseV1),
    ApprovalListResponse(CommandApprovalListResponseV4),
    Envelope(EventEnvelopeV1),
}
