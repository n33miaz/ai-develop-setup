import * as validators from 'virtual:wire-validators'
import type {
  CommandApprovalListResponseV3,
  CommandHarnessListResponseV1,
  CommandQueueGetResponseV2,
  CommandRequestV1,
  ContractLocalApiServerMessageV7,
} from './contracts'

function validate(urn: string, value: unknown): boolean {
  return validators[urn.replace(/[^a-zA-Z0-9]/g, '_')]?.(value) === true
}

export function isServerMessage(value: unknown): value is ContractLocalApiServerMessageV7 {
  if (!validate('urn:ads:contract:local-api.server-message:7', value)) return false
  const message = value as ContractLocalApiServerMessageV7
  if ('jsonrpc' in message) return true
  const event = message
  if (event.type.startsWith('urn:ads:event:session.')) {
    if (!validate(event.type, event)) return false
    const data = event.data as { session_id: string }
    if (event.stream !== `session/${data.session_id}`) return false
  }
  return true
}

export function isHarnessList(value: unknown): value is CommandHarnessListResponseV1 {
  return validate('urn:ads:command:harness.list.response:1', value)
}

export function isQueueGet(value: unknown): value is CommandQueueGetResponseV2 {
  return validate('urn:ads:command:queue.get.response:2', value)
}

export function isApprovalList(value: unknown): value is CommandApprovalListResponseV3 {
  return validate('urn:ads:command:approval.list.response:3', value)
}

export function isLocalCommand(command: CommandRequestV1): boolean {
  if (
    ![
      'auth.hello',
      'events.subscribe',
      'harness.list',
      'queue.get',
      'queue.create',
      'queue.start',
      'queue.stop',
      'approval.list',
      'approval.answer',
      'session.open',
      'session.write',
      'session.resize',
      'session.close',
    ].includes(command.method)
  )
    return false
  return (
    command.params._meta.origin === 'local' &&
    validate(`urn:ads:command:${command.method}:1`, command)
  )
}
