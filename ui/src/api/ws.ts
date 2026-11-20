import {
  CoreClientError,
  HarnessListError,
  isSubscriptionEndCode,
  type ConnectionState,
  type CoreClient,
  type SubscriptionEndCode,
} from './client'
import { newCommand } from './command'
import type {
  CommandHarnessListResponseV1,
  CommandRequestV1,
  CommandResponseV2,
  EventEnvelopeV1,
} from './contracts'
import { isHarnessList, isLocalCommand, isServerMessage } from './wire'

type Reply = CommandResponseV2 | CommandHarnessListResponseV1

export type CoreSocket = Pick<WebSocket, 'send' | 'close' | 'addEventListener'>

export interface WsClientOptions {
  url: string
  token: string
  socketFactory?: (url: string) => CoreSocket
  /** Prazo de `harness.list`; ao vencer, o pedido é esquecido e a consulta falha. */
  listTimeoutMs?: number
}

function accepted(response: Reply): boolean {
  return !isHarnessList(response) && 'result' in response
}

/** Quantos ids de `harness.list` vencidos são lembrados para descartar a resposta tardia. */
const EXPIRED_IDS = 32

export function createWsCoreClient({
  url,
  token,
  socketFactory = (url) => new WebSocket(url),
  listTimeoutMs = 10000,
}: WsClientOptions): CoreClient {
  let endpoint: URL
  try {
    endpoint = new URL(url)
  } catch {
    throw new Error('invalid local endpoint')
  }
  if (
    endpoint.protocol !== 'ws:' ||
    !['127.0.0.1', 'localhost'].includes(endpoint.hostname) ||
    endpoint.search ||
    endpoint.hash ||
    endpoint.username ||
    endpoint.password
  ) {
    throw new Error('invalid local endpoint')
  }
  let state: ConnectionState = 'disconnected'
  let socket: CoreSocket | null = null
  let connecting: Promise<void> | null = null
  let rejectConnection: ((error: Error) => void) | null = null
  let generation = 0
  const expired = new Set<string>()
  const listeners = new Set<(state: ConnectionState) => void>()
  const pending = new Map<
    string,
    { method: string; resolve: (response: Reply) => void; reject: (error: Error) => void }
  >()
  interface Subscription {
    stream: string
    seq: number
    onEvent: (event: EventEnvelopeV1) => void
    onEnd?: (code: SubscriptionEndCode) => void
  }
  const subscriptions = new Set<Subscription>()
  // O resultado final de uma assinatura chega como resposta ao id do `events.subscribe`, mesmo
  // depois de ela ter sido aceita; por isso o id continua ligado à assinatura enquanto ela vive.
  const subscriptionRequests = new Map<string, Subscription>()
  const unavailable = () => new CoreClientError('not_connected', 'core disconnected')
  const setState = (next: ConnectionState) => {
    if (state === next) return
    state = next
    for (const listener of [...listeners]) listener(next)
  }
  function disconnect() {
    const previous = socket
    socket = null
    connecting = null
    rejectConnection?.(unavailable())
    rejectConnection = null
    subscriptions.clear()
    subscriptionRequests.clear()
    expired.clear()
    for (const request of pending.values()) request.reject(unavailable())
    pending.clear()
    setState('disconnected')
    previous?.close()
  }
  function endSubscription(id: string, code: SubscriptionEndCode) {
    const subscription = subscriptionRequests.get(id)
    subscriptionRequests.delete(id)
    if (!subscription || !subscriptions.has(subscription)) return
    subscriptions.delete(subscription)
    subscription.onEnd?.(code)
  }
  function transmit(command: CommandRequestV1): Promise<Reply> {
    if (!isLocalCommand(command)) return Promise.reject(new Error('invalid local command'))
    if (pending.has(command.id)) return Promise.reject(new Error('duplicate request id'))
    return new Promise((resolve, reject) => {
      pending.set(command.id, { method: command.method, resolve, reject })
      try {
        socket?.send(JSON.stringify(command))
      } catch {
        disconnect()
      }
    })
  }
  return {
    getState: () => state,
    getGeneration: () => generation,
    onStateChange(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    connect() {
      if (state === 'connected') return Promise.resolve()
      if (connecting) return connecting
      setState('connecting')
      const attempt = new Promise<void>((resolve, reject) => {
        rejectConnection = reject
        let current: CoreSocket
        try {
          current = socketFactory(endpoint.href)
          socket = current
        } catch {
          disconnect()
          reject(unavailable())
          return
        }
        const fail = () => {
          if (socket !== current) return
          disconnect()
          reject(unavailable())
        }
        current.addEventListener('close', fail)
        current.addEventListener('error', fail)
        current.addEventListener('open', () => {
          if (socket !== current) return
          void transmit(newCommand('auth.hello', { token })).then(
            (response) => {
              if (socket !== current) return
              if (!accepted(response)) {
                fail()
                return
              }
              connecting = null
              rejectConnection = null
              generation += 1
              setState('connected')
              resolve()
            },
            () => {
              fail()
            },
          )
        })
        current.addEventListener('message', (message) => {
          if (socket !== current) return
          let value: unknown
          try {
            value = JSON.parse(String(message.data))
          } catch {
            fail()
            return
          }
          // A resposta a um pedido vencido não tem mais quem a espere: some sem derrubar a conexão.
          if (
            typeof value === 'object' &&
            value !== null &&
            'jsonrpc' in value &&
            'id' in value &&
            typeof value.id === 'string' &&
            expired.delete(value.id)
          )
            return
          if (!isServerMessage(value)) {
            fail()
            return
          }
          if ('jsonrpc' in value) {
            const response = value
            const request = pending.get(response.id)
            if ('error' in response) {
              response.error = { code: response.error.code, message: 'command rejected' }
            } else if (request && isHarnessList(response) !== (request.method === 'harness.list')) {
              // Cada resposta tem a forma do pedido: a lista só responde a harness.list.
              fail()
              return
            }
            pending.delete(response.id)
            // Resposta de assinatura ativa, inicial ou tardia: -32004 e -32013 encerram só ela;
            // outro erro não tem explicação e derruba a conexão. Assinatura cancelada não chega aqui.
            if (subscriptionRequests.has(response.id) && 'error' in response) {
              const { code } = response.error
              if (!isSubscriptionEndCode(code)) {
                fail()
                return
              }
              endSubscription(response.id, code)
            }
            request?.resolve(response)
          } else {
            const event = value
            if (
              !Number.isSafeInteger(event.seq) ||
              event.seq < 1 ||
              typeof event.stream !== 'string'
            ) {
              fail()
              return
            }
            for (const subscription of [...subscriptions]) {
              if (state !== 'connected' || !subscriptions.has(subscription)) continue
              if (event.stream !== subscription.stream || event.seq <= subscription.seq) continue
              // O servidor entrega o replay em ordem; um buraco encerra a conexão sem perder dados silenciosamente.
              if (subscription.seq !== 0 && event.seq !== subscription.seq + 1) {
                fail()
                return
              }
              subscription.seq = event.seq
              subscription.onEvent(event)
            }
          }
        })
      })
      connecting = attempt
      void attempt.catch(() => {
        if (connecting === attempt) connecting = null
      })
      return attempt
    },
    disconnect,
    send(command) {
      if (state !== 'connected') return Promise.reject(unavailable())
      if (command.method === 'harness.list') return Promise.reject(new Error('use listHarnesses'))
      return transmit(command) as Promise<CommandResponseV2>
    },
    listHarnesses() {
      if (state !== 'connected') return Promise.reject(unavailable())
      const command = newCommand('harness.list')
      let timer: ReturnType<typeof setTimeout> | undefined
      const deadline = new Promise<never>((_, reject) => {
        timer = setTimeout(() => {
          const request = pending.get(command.id)
          pending.delete(command.id)
          if (request) {
            expired.add(command.id)
            for (const id of expired) {
              if (expired.size <= EXPIRED_IDS) break
              expired.delete(id)
            }
          }
          request?.reject(new HarnessListError('timeout'))
          reject(new HarnessListError('timeout'))
        }, listTimeoutMs)
      })
      return Promise.race([transmit(command), deadline])
        .then((response) => {
          if (isHarnessList(response)) return response.result
          // -32601: núcleo anterior ao método.
          throw new HarnessListError(response.error?.code === -32601 ? 'unsupported' : 'refused')
        })
        .finally(() => {
          clearTimeout(timer)
        })
    },
    subscribe(stream, afterSeq, onEvent, onEnd) {
      if (state !== 'connected') throw unavailable()
      const subscription: Subscription = { stream, seq: afterSeq, onEvent }
      if (onEnd) subscription.onEnd = onEnd
      const command = newCommand('events.subscribe', { stream, after_seq: afterSeq })
      subscriptions.add(subscription)
      subscriptionRequests.set(command.id, subscription)
      void transmit(command).catch(() => undefined)
      return () => {
        subscriptions.delete(subscription)
        subscriptionRequests.delete(command.id)
        pending.delete(command.id)
      }
    },
  }
}
