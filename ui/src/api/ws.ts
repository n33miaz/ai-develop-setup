import { CoreClientError, type ConnectionState, type CoreClient } from './client'
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
  const listeners = new Set<(state: ConnectionState) => void>()
  const pending = new Map<
    string,
    { method: string; resolve: (response: Reply) => void; reject: (error: Error) => void }
  >()
  const subscriptions = new Set<{
    stream: string
    seq: number
    onEvent: (event: EventEnvelopeV1) => void
  }>()
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
    for (const request of pending.values()) request.reject(unavailable())
    pending.clear()
    setState('disconnected')
    previous?.close()
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
            const request = pending.get(response.id)
            if ('error' in response) {
              response.error = { code: response.error.code, message: 'command rejected' }
            } else if (request && isHarnessList(response) !== (request.method === 'harness.list')) {
              // Cada resposta tem a forma do pedido: a lista só responde a harness.list.
              fail()
              return
            }
            pending.delete(response.id)
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
      const expired = new Promise<never>((_, reject) => {
        timer = setTimeout(() => {
          const request = pending.get(command.id)
          pending.delete(command.id)
          request?.reject(new Error('core timeout'))
          reject(new Error('core timeout'))
        }, listTimeoutMs)
      })
      return Promise.race([transmit(command), expired])
        .then((response) => {
          if (!isHarnessList(response)) throw new Error('command rejected')
          return response.result
        })
        .finally(() => {
          clearTimeout(timer)
        })
    },
    subscribe(stream, afterSeq, onEvent) {
      if (state !== 'connected') throw unavailable()
      const subscription = { stream, seq: afterSeq, onEvent }
      subscriptions.add(subscription)
      void transmit(newCommand('events.subscribe', { stream, after_seq: afterSeq })).then(
        (response) => {
          if (!accepted(response)) disconnect()
        },
        () => undefined,
      )
      return () => {
        subscriptions.delete(subscription)
      }
    },
  }
}
