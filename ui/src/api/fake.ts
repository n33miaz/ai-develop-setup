import { CoreClientError, type ConnectionState, type CoreClient, type Unsubscribe } from './client'
import type {
  CommandRequestV1,
  CommandResponseV1,
  CommonActorV1,
  EventDraft,
  EventEnvelopeV1,
} from './contracts'
import { newUlid } from './ulid'

/** Cliente falso em memória: dá para testar a UI sem núcleo. Não simula rede nem atraso. */
export interface FakeCoreClient extends CoreClient {
  /** Grava um evento no fluxo (como se o núcleo o tivesse produzido) e entrega aos assinantes. */
  emit(draft: EventDraft): EventEnvelopeV1
  /** Comandos que tiveram efeito, na ordem; um reenvio com a mesma chave não entra de novo. */
  applied(): readonly CommandRequestV1[]
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
  const applied: CommandRequestV1[] = []
  const results = new Map<string, { accepted: true }>()

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
      let result = results.get(key)
      if (!result) {
        // O contrato deixa `result` aberto; o falso só confirma o aceite, como o exemplo do Schema.
        result = { accepted: true }
        results.set(key, result)
        applied.push(command)
      }
      const response: CommandResponseV1 = { jsonrpc: '2.0', id: command.id, result }
      return Promise.resolve(response)
    },

    emit: append,

    applied: () => [...applied],
  }
}
