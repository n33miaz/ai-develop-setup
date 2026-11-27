import type { EventEnvelopeV1 } from './contracts'

export const DEFAULT_REPLAY_EVENTS = 256
export const DEFAULT_REPLAY_BYTES = 4 * 1024 * 1024

export interface ReplayBuffer {
  /** `after_seq` pedido ao núcleo. */
  start: number
  /** Maior `seq` recebido. */
  last: number
  events: { event: EventEnvelopeV1; size: number }[]
  bytes: number
}

/** Limite de replay válido (inteiro de 1 em diante) ou o padrão. */
export function replayLimit(value: number | undefined, fallback: number): number {
  // Com 0 nada ficaria guardado: um terminal remontado ficaria vazio e uma nova assinatura depois de
  // reconectar daria por encerrada uma sessão viva.
  return value !== undefined && Number.isSafeInteger(value) && value >= 1 ? value : fallback
}

/** Guarda o evento recebido em ordem, descartando os mais antigos acima dos tetos. */
export function remember(
  buffer: ReplayBuffer,
  event: EventEnvelopeV1,
  size: number,
  replayEvents: number,
  replayBytes: number,
) {
  if (event.seq <= (buffer.events.at(-1)?.event.seq ?? 0)) return
  buffer.events.push({ event, size })
  buffer.bytes += size
  while (buffer.events.length > replayEvents || buffer.bytes > replayBytes) {
    buffer.bytes -= buffer.events.shift()?.size ?? 0
  }
}

/** Os eventos guardados bastam para um ouvinte que começa depois de `afterSeq`. */
export function covers(buffer: ReplayBuffer, afterSeq: number): boolean {
  if (afterSeq >= buffer.last) return true
  if (afterSeq < buffer.start) return false
  // A partir de 0 o núcleo também só reenviaria o que ainda retém.
  if (afterSeq === 0 && buffer.start === 0) return true
  const oldest = buffer.events[0]?.event.seq
  return oldest !== undefined && oldest <= afterSeq + 1
}
