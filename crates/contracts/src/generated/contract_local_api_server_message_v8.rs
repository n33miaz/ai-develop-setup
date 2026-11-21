// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.server-message.v8.schema.json. Não edite à mão.

use serde::{Deserialize, Serialize};

use super::command_approval_list_response_v3::CommandApprovalListResponseV3;
use super::command_harness_list_response_v1::CommandHarnessListResponseV1;
use super::command_queue_configured_response_v1::CommandQueueConfiguredResponseV1;
use super::command_queue_get_response_v2::CommandQueueGetResponseV2;
use super::command_response_v2::CommandResponseV2;
use super::event_envelope_v1::EventEnvelopeV1;

/// Mensagem do núcleo à interface. Tudo o que o núcleo manda à interface pelo WebSocket da API local: a resposta a um comando (command.response:2), a resposta aceita de harness.list (command.harness.list.response:1), de queue.get (command.queue.get.response:2), de queue.configured (command.queue.configured.response:1) ou de approval.list (command.approval.list.response:3), ou um evento de um stream assinado (event.envelope:1). Cada mensagem é uma das seis, nunca mais de uma. A versão 8 acrescenta à :7 a resposta de queue.configured.
///
/// `urn:ads:contract:local-api.server-message:8`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiServerMessageV8 {
    Response(CommandResponseV2),
    HarnessListResponse(CommandHarnessListResponseV1),
    QueueGetResponse(CommandQueueGetResponseV2),
    QueueConfiguredResponse(CommandQueueConfiguredResponseV1),
    ApprovalListResponse(CommandApprovalListResponseV3),
    Envelope(EventEnvelopeV1),
}
