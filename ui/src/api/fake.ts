import { CoreClientError, type ConnectionState, type CoreClient, type Unsubscribe } from './client'
import type {
  CommandResponseV2,
  CommonActorV1,
  EventDraft,
  EventEnvelopeV1,
  HarnessCatalog,
} from './contracts'
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

function deepFreeze<T>(value: T): T {
  if (typeof value === 'object' && value !== null && !Object.isFrozen(value)) {
    Object.freeze(value)
    for (const inner of Object.values(value)) deepFreeze(inner)
  }
  return value
}

/** `catalog` é o que `listHarnesses` responde; o padrão é nenhuma ferramenta. */
export function createFakeCoreClient(
  catalog: HarnessCatalog = { harnesses: [], workdir_aliases: [] },
): FakeCoreClient {
  let state: ConnectionState = 'disconnected'
  const stateListeners = new Set<(state: ConnectionState) => void>()
  const logs = new Map<string, EventEnvelopeV1[]>()
  const subscriptions = new Set<Subscription>()
  const acceptedKeys = new Set<string>()
  let connecting: Promise<void> | null = null
  let generation = 0

  function setState(next: ConnectionState): void {
    if (state === next) return
    state = next
    for (const listener of [...stateListeners]) listener(state)
  }

  function notConnected(): CoreClientError {
    return new CoreClientError('not_connected', 'o núcleo não está conectado')
  }

  // Uma assinatura vale enquanto for ela mesma (por identidade) que está registrada e a conexão
  // estiver ativa. Um callback pode desligar a conexão ou cancelar uma assinatura no meio da entrega.
  function isLive(subscription: Subscription): boolean {
    return state === 'connected' && subscriptions.has(subscription)
  }

  function deliver(subscription: Subscription): void {
    const log = logs.get(subscription.stream) ?? []
    for (const event of log) {
      // Conferido antes de cada callback: o anterior pode ter encerrado esta assinatura.
      if (!isLive(subscription)) return
      if (event.seq > subscription.lastDelivered) {
        subscription.lastDelivered = event.seq
        subscription.onEvent(event)
      }
    }
  }

  function append(draft: EventDraft): EventEnvelopeV1 {
    const log = logs.get(draft.stream) ?? []
    logs.set(draft.stream, log)
    // Fato gravado não muda: o evento é uma cópia congelada, que nem o autor do rascunho nem um
    // assinante consegue alterar para os outros.
    const event: EventEnvelopeV1 = deepFreeze({
      id: newUlid(),
      type: draft.type,
      ts: new Date().toISOString(),
      stream: draft.stream,
      seq: log.length + 1,
      actor: { ...(draft.actor ?? FAKE_ACTOR) },
      cause: draft.cause ?? null,
      data_class: draft.data_class ?? 'internal',
      data: structuredClone(draft.data),
    })
    log.push(event)
    for (const subscription of [...subscriptions]) {
      if (subscription.stream === draft.stream) deliver(subscription)
    }
    return event
  }

  return {
    getState: () => state,
    getGeneration: () => generation,

    onStateChange(listener) {
      stateListeners.add(listener)
      return () => stateListeners.delete(listener)
    },

    connect() {
      if (state === 'connected') return Promise.resolve()
      if (connecting) return connecting
      setState('connecting')
      const attempt = Promise.resolve().then(() => {
        // `disconnect()` no meio do caminho cancela a tentativa: ela não volta a conectar.
        if (connecting !== attempt) throw notConnected()
        connecting = null
        generation += 1
        setState('connected')
      })
      connecting = attempt
      return attempt
    },

    // A conexão leva as assinaturas junto, inclusive a que está no meio de uma entrega; os eventos
    // continuam sendo gravados e quem reconectar retoma pelo último `seq` que viu.
    disconnect() {
      connecting = null
      subscriptions.clear()
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

    listHarnesses() {
      if (state !== 'connected') return Promise.reject(notConnected())
      return Promise.resolve(structuredClone(catalog))
    },

    emit: append,
  }
}
