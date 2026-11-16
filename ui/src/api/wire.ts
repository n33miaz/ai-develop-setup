import * as validators from 'virtual:wire-validators'
import type { ContractLocalApiServerMessageV1, CommandRequestV1 } from './contracts'

function validate(urn: string, value: unknown): boolean {
  return validators[urn.replace(/[^a-zA-Z0-9]/g, '_')]?.(value) === true
}

export function isServerMessage(value: unknown): value is ContractLocalApiServerMessageV1 {
  if (!validate('urn:ads:contract:local-api.server-message:1', value)) return false
  const message = value as ContractLocalApiServerMessageV1
  if ('jsonrpc' in message) return true
  const event = message
  if (event.type.startsWith('urn:ads:event:session.')) {
    if (!validate(event.type, event)) return false
    const data = event.data as { session_id: string }
    if (event.stream !== `session/${data.session_id}`) return false
  }
  return true
}

export function isLocalCommand(command: CommandRequestV1): boolean {
  if (
    ![
      'auth.hello',
      'events.subscribe',
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
