import type { CommandRequestV1 } from './contracts'
import { newIdempotencyKey } from './idempotency'
import { newUlid } from './ulid'

/** Comando local novo, com chave de idempotência própria. Para reenviar, reenvie o mesmo objeto. */
export function newCommand(method: string, params: Record<string, unknown> = {}): CommandRequestV1 {
  return {
    jsonrpc: '2.0',
    id: newUlid(),
    method,
    params: { ...params, _meta: { idempotency_key: newIdempotencyKey(), origin: 'local' } },
  }
}
