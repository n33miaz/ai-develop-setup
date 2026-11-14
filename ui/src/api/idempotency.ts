/** Chave nova para um comando. Reenvios do mesmo comando devem reusar a mesma chave. */
export function newIdempotencyKey(): string {
  return crypto.randomUUID()
}
