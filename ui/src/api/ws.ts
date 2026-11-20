import {
  CoreClientError,
  isSubscriptionEndCode,
  type ConnectionState,
  type CoreClient,
  type SubscriptionEndCode,
} from './client'
import { newCommand } from './command'
import type { CommandRequestV1, CommandResponseV2, EventEnvelopeV1 } from './contracts'
import { isLocalCommand, isServerMessage } from './wire'

export type CoreSocket = Pick<WebSocket, 'send' | 'close' | 'addEventListener'>

export interface WsClientOptions {
  url: string
  token: string
  socketFactory?: (url: string) => CoreSocket
}

export function createWsCoreClient({
  url,
  token,
  socketFactory = (url) => new WebSocket(url),
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
  const listeners = new Set<(state: ConnectionState) => void>()
  const pending = new Map<
    string,
    { resolve: (response: CommandResponseV2) => void; reject: (error: Error) => void }
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
  function transmit(command: CommandRequestV1): Promise<CommandResponseV2> {
    if (!isLocalCommand(command)) return Promise.reject(new Error('invalid local command'))
    if (pending.has(command.id)) return Promise.reject(new Error('duplicate request id'))
    return new Promise((resolve, reject) => {
      pending.set(command.id, { resolve, reject })
      try {
        socket?.send(JSON.stringify(command))
      } catch {
        disconnect()
      }
    })
  }
  return {
    getState: () => state,
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
              if (!response.result) {
                fail()
                return
              }
              connecting = null
              rejectConnection = null
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
          if (!isServerMessage(value)) {
            fail()
            return
          }
          if ('jsonrpc' in value) {
            const response = value
            if (response.error)
              response.error = { code: response.error.code, message: 'command rejected' }
            const request = pending.get(response.id)
            pending.delete(response.id)
            if (subscriptionRequests.has(response.id) && response.error) {
              // Só esta assinatura termina; outro erro tardio dela não tem como ser explicado.
              if (isSubscriptionEndCode(response.error.code))
                endSubscription(response.id, response.error.code)
              else if (!request) {
                fail()
                return
              }
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
      return state === 'connected' ? transmit(command) : Promise.reject(unavailable())
    },
    subscribe(stream, afterSeq, onEvent, onEnd) {
      if (state !== 'connected') throw unavailable()
      const subscription: Subscription = { stream, seq: afterSeq, onEvent }
      if (onEnd) subscription.onEnd = onEnd
      const command = newCommand('events.subscribe', { stream, after_seq: afterSeq })
      subscriptions.add(subscription)
      subscriptionRequests.set(command.id, subscription)
      void transmit(command).then(
        (response) => {
          if (response.error && !isSubscriptionEndCode(response.error.code)) disconnect()
        },
        () => undefined,
      )
      return () => {
        subscriptions.delete(subscription)
        subscriptionRequests.delete(command.id)
      }
    },
  }
}
