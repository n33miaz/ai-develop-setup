import type { CommonUlidV1 } from './contracts'
import { newUlid } from './ulid'

/** Chave nova para um comando. Reenvios do mesmo comando devem reusar a mesma chave. */
export function newIdempotencyKey(): CommonUlidV1 {
  return newUlid()
}
