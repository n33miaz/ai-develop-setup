import { CoreClientError, type ConnectionState, type CoreClient, type Unsubscribe } from './client'
import type { CommandResponseV2, CommonActorV1, EventDraft, EventEnvelopeV1 } from './contracts'
import { newUlid } from './ulid'

/** Cliente falso em memória: dá para testar a UI sem núcleo. Não simula rede nem atraso. */
export interface FakeCoreClient extends CoreClient {
  /** Grava um evento no fluxo (como se o núcleo o tivesse produzido) e entrega aos assinantes. */
  emit(draft: EventDraft): EventEnvelopeV1
}

interface Subscription {
  stream: string
  lastDelivered: number
  onEvent: (event: EventEnvelopeV1) => void
}

const FAKE_ACTOR: CommonActorV1 = { kind: 'daemon', id: 'fake-core' }

export function createFakeCoreClient(): FakeCoreClient {
  let state: ConnectionState = 'disconnected'
  const stateListeners = new Set<(state: ConnectionState) => void>()
  const logs = new Map<string, EventEnvelopeV1[]>()
  const subscriptions = new Set<Subscription>()
  const acceptedKeys = new Set<string>()

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

  function append(draft: EventDraft): EventEnvelopeV1 {
    const log = logs.get(draft.stream) ?? []
    logs.set(draft.stream, log)
    const event: EventEnvelopeV1 = {
      id: newUlid(),
      type: draft.type,
      ts: new Date().toISOString(),
      stream: draft.stream,
      seq: log.length + 1,
      actor: draft.actor ?? FAKE_ACTOR,
      cause: draft.cause ?? null,
      data_class: draft.data_class ?? 'internal',
      data: draft.data,
    }
    log.push(event)
    for (const subscription of [...subscriptions]) {
      if (subscription.stream === draft.stream) deliver(subscription)
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

    send(command) {
      if (state !== 'connected') return Promise.reject(notConnected())
      const key = command.params._meta.idempotency_key
      // O falso não tem efeito a repetir: só lembra a chave para marcar a repetição.
      const replayed = acceptedKeys.has(key)
      acceptedKeys.add(key)
      const response: CommandResponseV2 = {
        jsonrpc: '2.0',
        id: command.id,
        result: { accepted: true, replayed },
      }
      return Promise.resolve(response)
    },

    emit: append,
  }
}
