// Ponto único de importação dos tipos das mensagens trocadas com o núcleo.
// Todos vêm dos tipos gerados dos JSON Schemas (`contracts/generated/ts`); nenhum é escrito à mão.
// O resto da UI importa daqui, nunca direto de `contracts/`. Só tipos: nada disto entra no bundle.
import type { EventApprovalDeniedV2 } from '../../../contracts/generated/ts/event.approval.denied.v2'
import type { EventApprovalExpiredV2 } from '../../../contracts/generated/ts/event.approval.expired.v2'
import type { EventApprovalGrantedV2 } from '../../../contracts/generated/ts/event.approval.granted.v2'
import type { EventApprovalRequestedV2 } from '../../../contracts/generated/ts/event.approval.requested.v2'
import type { EventEnvelopeV1 } from '../../../contracts/generated/ts/event.envelope.v1'
import type { EventHostAlertAckedV1 } from '../../../contracts/generated/ts/event.host.alert_acked.v1'
import type { EventHostAlertRaisedV1 } from '../../../contracts/generated/ts/event.host.alert_raised.v1'
import type { EventHostSampleRecordedV1 } from '../../../contracts/generated/ts/event.host.sample_recorded.v1'
import type { EventSessionActivityChangedV1 } from '../../../contracts/generated/ts/event.session.activity_changed.v1'
import type { EventSessionCreatedV1 } from '../../../contracts/generated/ts/event.session.created.v1'
import type { EventSessionEndedV1 } from '../../../contracts/generated/ts/event.session.ended.v1'
import type { EventSessionOutputV1 } from '../../../contracts/generated/ts/event.session.output.v1'
import type { EventSessionPermissionAskedV1 } from '../../../contracts/generated/ts/event.session.permission_asked.v1'
import type { EventSessionResizedV1 } from '../../../contracts/generated/ts/event.session.resized.v1'
import type { EventTaskCancelledV1 } from '../../../contracts/generated/ts/event.task.cancelled.v1'
import type { EventTaskCompletedV1 } from '../../../contracts/generated/ts/event.task.completed.v1'
import type { EventTaskCreatedV1 } from '../../../contracts/generated/ts/event.task.created.v1'
import type { EventTaskFailedV1 } from '../../../contracts/generated/ts/event.task.failed.v1'
import type { EventTaskStartedV1 } from '../../../contracts/generated/ts/event.task.started.v1'
import type { EventUsageReconciledV1 } from '../../../contracts/generated/ts/event.usage.reconciled.v1'
import type { EventUsageRecordedV1 } from '../../../contracts/generated/ts/event.usage.recorded.v1'

import type { CommandHarnessListResponseV1 } from '../../../contracts/generated/ts/command.harness.list.response.v1'

export type { CommandRequestV1 } from '../../../contracts/generated/ts/command.request.v1'
export type { CommandResponseV2 } from '../../../contracts/generated/ts/command.response.v2'
export type { CommandHarnessListResponseV1 }
export type { ContractLocalApiServerMessageV2 } from '../../../contracts/generated/ts/contract.local-api.server-message.v2'
export type {
  CommonActorV1,
  CommonDataClassV1,
  CommonIdV1,
  CommonTimestampV1,
  CommonUlidV1,
} from '../../../contracts/generated/ts/event.envelope.v1'
export type {
  EventApprovalDeniedV2,
  EventApprovalExpiredV2,
  EventApprovalGrantedV2,
  EventApprovalRequestedV2,
  EventEnvelopeV1,
  EventHostAlertAckedV1,
  EventHostAlertRaisedV1,
  EventHostSampleRecordedV1,
  EventSessionActivityChangedV1,
  EventSessionCreatedV1,
  EventSessionEndedV1,
  EventSessionOutputV1,
  EventSessionPermissionAskedV1,
  EventSessionResizedV1,
  EventTaskCancelledV1,
  EventTaskCompletedV1,
  EventTaskCreatedV1,
  EventTaskFailedV1,
  EventTaskStartedV1,
  EventUsageReconciledV1,
  EventUsageRecordedV1,
}

export type SessionEvent =
  | EventSessionActivityChangedV1
  | EventSessionCreatedV1
  | EventSessionEndedV1
  | EventSessionOutputV1
  | EventSessionPermissionAskedV1
  | EventSessionResizedV1

export type TaskEvent =
  | EventTaskCancelledV1
  | EventTaskCompletedV1
  | EventTaskCreatedV1
  | EventTaskFailedV1
  | EventTaskStartedV1

/**
 * Só as `:2`, que dizem de qual ação são (`action_id`). O núcleo escreve só elas e recusa as `:1`
 * na leitura.
 */
export type ApprovalEvent =
  EventApprovalDeniedV2 | EventApprovalExpiredV2 | EventApprovalGrantedV2 | EventApprovalRequestedV2

export type UsageEvent = EventUsageReconciledV1 | EventUsageRecordedV1

export type HostEvent = EventHostAlertAckedV1 | EventHostAlertRaisedV1 | EventHostSampleRecordedV1

/** O que o núcleo sabe abrir: só nomes de ferramenta e apelidos de pasta. */
export type HarnessCatalog = CommandHarnessListResponseV1['result']

/** Os eventos que a interface conhece hoje; os demais chegam só como envelope. */
export type KnownEvent = SessionEvent | TaskEvent | ApprovalEvent | UsageEvent | HostEvent

/**
 * O que quem produz um evento informa; `id`, `ts` e `seq` são de quem grava. Não é mensagem de
 * contrato: é um recorte dos tipos gerados, usado pelo cliente falso e pelos dados de exemplo.
 */
export type EventDraft<E extends EventEnvelopeV1 = EventEnvelopeV1> = E extends EventEnvelopeV1
  ? Pick<E, 'type' | 'stream' | 'data'> & Partial<Pick<E, 'actor' | 'cause' | 'data_class'>>
  : never
