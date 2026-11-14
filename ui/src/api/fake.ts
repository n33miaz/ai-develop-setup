import { CoreClientError, type CoreClient, type Unsubscribe } from './client'
import type { CommandAck, CoreCommand, CoreEvent, ConnectionState, Seq } from './types'

/** Cliente falso em memória: dá para testar a UI sem núcleo. Não simula rede nem atraso. */
export interface FakeCoreClient extends CoreClient {
  /** Grava um evento no fluxo (como se o núcleo o tivesse produzido) e entrega aos assinantes. */
  emit(stream: string, type: string, data?: unknown): CoreEvent
}

interface Subscription {
  stream: string
  lastDelivered: Seq
  onEvent: (event: CoreEvent) => void
}

export function createFakeCoreClient(): FakeCoreClient {
  let state: ConnectionState = 'disconnected'
  let counter = 0
  const stateListeners = new Set<(state: ConnectionState) => void>()
  const logs = new Map<string, CoreEvent[]>()
  const subscriptions = new Set<Subscription>()
  const acks = new Map<string, CommandAck>()

  function setState(next: ConnectionState): void {
    if (state === next) return
    state = next
    for (const listener of [...stateListeners]) listener(state)
  }

  function notConnected(): CoreClientError {
    return new CoreClientError('not_connected', 'o núcleo não está conectado')
  }

  function deliver(subscription: Subscription): void {
    const log = logs.get(subscription.stream) ?? []
    for (const event of log) {
      if (event.seq > subscription.lastDelivered) {
        subscription.lastDelivered = event.seq
        subscription.onEvent(event)
      }
    }
  }

  function append(stream: string, type: string, data: unknown, cause?: string): CoreEvent {
    const log = logs.get(stream) ?? []
    logs.set(stream, log)
    counter += 1
    const event: CoreEvent = {
      id: `fake-${String(counter)}`,
      type,
      ts: new Date().toISOString(),
      stream,
      seq: log.length + 1,
      data,
      ...(cause === undefined ? {} : { cause }),
    }
    log.push(event)
    for (const subscription of [...subscriptions]) {
      if (subscription.stream === stream) deliver(subscription)
    }
    return event
  }

  return {
    getState: () => state,

    onStateChange(listener) {
      stateListeners.add(listener)
      return () => stateListeners.delete(listener)
    },

    connect() {
      if (state === 'connected') return Promise.resolve()
      setState('connecting')
      return Promise.resolve().then(() => {
        setState('connected')
      })
    },

    disconnect() {
      setState('disconnected')
    },

    subscribe(stream, afterSeq, onEvent): Unsubscribe {
      if (state !== 'connected') throw notConnected()
      const subscription: Subscription = { stream, lastDelivered: afterSeq, onEvent }
      subscriptions.add(subscription)
      deliver(subscription)
      return () => subscriptions.delete(subscription)
    },

    send(command: CoreCommand, idempotencyKey) {
      if (state !== 'connected') return Promise.reject(notConnected())
      const previous = acks.get(idempotencyKey)
      if (previous) return Promise.resolve({ ...previous, duplicate: true })
      const event = append('commands', `command.${command.method}`, command.params, idempotencyKey)
      const ack: CommandAck = {
        idempotencyKey,
        duplicate: false,
        stream: event.stream,
        seq: event.seq,
      }
      acks.set(idempotencyKey, ack)
      return Promise.resolve(ack)
    },

    emit: (stream, type, data) => append(stream, type, data),
  }
}
