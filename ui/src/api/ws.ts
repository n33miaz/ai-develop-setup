import { METHOD_NOT_FOUND } from '../../../contracts/generated/ts/contract.local-api.error-code.v1.constants'
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
  /**
   * Eventos recentes guardados por stream para quem assina depois, sem nova assinatura no núcleo.
   * Deve andar junto com `events_per_stream` do daemon (padrão 256 nos dois). Mínimo 1: um valor
   * abaixo disso, ou que não seja inteiro, volta ao padrão.
   */
  replayEvents?: number
  /**
   * Teto do que fica guardado por stream, contado em unidades UTF-16 das mensagens recebidas
   * (igual a bytes para texto ASCII). Mínimo 1, como `replayEvents`.
   */
  replayBytes?: number
}

function accepted(response: Reply): boolean {
  return !isHarnessList(response) && 'result' in response
}

/** Quantos ids de `harness.list` vencidos são lembrados para descartar a resposta tardia. */
const EXPIRED_IDS = 32

const DEFAULT_REPLAY_EVENTS = 256
const DEFAULT_REPLAY_BYTES = 4 * 1024 * 1024

/** Limite de replay válido (inteiro de 1 em diante) ou o padrão. */
function replayLimit(value: number | undefined, fallback: number): number {
  return value !== undefined && Number.isSafeInteger(value) && value >= 1 ? value : fallback
}

export function createWsCoreClient({
  url,
  token,
  socketFactory = (url) => new WebSocket(url),
  listTimeoutMs = 10000,
  replayEvents: configuredEvents,
  replayBytes: configuredBytes,
}: WsClientOptions): CoreClient {
  // Com 0 nada ficaria guardado: um terminal remontado ficaria vazio e uma nova assinatura depois de
  // reconectar daria por encerrada uma sessão viva.
  const replayEvents = replayLimit(configuredEvents, DEFAULT_REPLAY_EVENTS)
  const replayBytes = replayLimit(configuredBytes, DEFAULT_REPLAY_BYTES)
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
  interface Listener {
    seq: number
    onEvent: (event: EventEnvelopeV1) => void
    onEnd?: (code: SubscriptionEndCode) => void
  }
  // O protocolo não cancela assinaturas: uma assinatura no núcleo por stream e conexão serve a todos
  // os ouvintes locais e continua depois que o último sai, guardando os eventos recentes para quem
  // vier depois. Um ouvinte que pede eventos que já não estão guardados recebe só o próprio fim.
  interface Shared {
    stream: string
    /** `after_seq` pedido ao núcleo. */
    start: number
    /** Maior `seq` recebido. */
    last: number
    events: { event: EventEnvelopeV1; size: number }[]
    bytes: number
    listeners: Set<Listener>
  }
  const streams = new Map<string, Shared>()
  // O resultado final de uma assinatura chega como resposta ao id do `events.subscribe`, mesmo
  // depois de ela ter sido aceita; por isso o id continua ligado ao stream enquanto ele vive.
  const subscriptionRequests = new Map<string, Shared>()
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
    streams.clear()
    subscriptionRequests.clear()
    expired.clear()
    for (const request of pending.values()) request.reject(unavailable())
    pending.clear()
    setState('disconnected')
    previous?.close()
  }
  /** Encerra o stream inteiro: o núcleo termina juntas as assinaturas da mesma encarnação. */
  function endStream(shared: Shared, code?: SubscriptionEndCode) {
    if (streams.get(shared.stream) !== shared) return
    streams.delete(shared.stream)
    for (const [id, owner] of subscriptionRequests) {
      if (owner === shared) subscriptionRequests.delete(id)
    }
    const listeners = [...shared.listeners]
    shared.listeners.clear()
    if (code === undefined) return
    for (const listener of listeners) listener.onEnd?.(code)
  }
  /** Entrega um evento a um ouvinte; false quando há buraco na sequência. */
  function deliver(listener: Listener, event: EventEnvelopeV1): boolean {
    if (event.seq <= listener.seq) return true
    // O servidor entrega o replay em ordem; um buraco encerra a conexão sem perder dados silenciosamente.
    if (listener.seq !== 0 && event.seq !== listener.seq + 1) return false
    listener.seq = event.seq
    listener.onEvent(event)
    return true
  }
  /** Guarda o evento recebido em ordem, descartando os mais antigos acima dos tetos. */
  function remember(shared: Shared, event: EventEnvelopeV1, size: number) {
    if (event.seq <= (shared.events.at(-1)?.event.seq ?? 0)) return
    shared.events.push({ event, size })
    shared.bytes += size
    while (shared.events.length > replayEvents || shared.bytes > replayBytes) {
      shared.bytes -= shared.events.shift()?.size ?? 0
    }
  }
  /** Os eventos guardados bastam para um ouvinte que começa depois de `afterSeq`. */
  function covers(shared: Shared, afterSeq: number): boolean {
    if (afterSeq >= shared.last) return true
    if (afterSeq < shared.start) return false
    // A partir de 0 o núcleo também só reenviaria o que ainda retém.
    if (afterSeq === 0 && shared.start === 0) return true
    const oldest = shared.events[0]?.event.seq
    return oldest !== undefined && oldest <= afterSeq + 1
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
            // Resposta de assinatura, inicial ou tardia: -32004 e -32013 encerram só o stream dela;
            // outro erro não tem explicação e derruba a conexão, a menos que ninguém mais ouça o stream.
            const shared = subscriptionRequests.get(response.id)
            if (shared && 'error' in response) {
              const { code } = response.error
              if (shared.listeners.size === 0) {
                endStream(shared)
              } else if (!isSubscriptionEndCode(code)) {
                fail()
                return
              } else {
                endStream(shared, code)
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
            const shared = streams.get(event.stream)
            if (!shared) return
            remember(shared, event, String(message.data).length)
            shared.last = Math.max(shared.last, event.seq)
            for (const listener of [...shared.listeners]) {
              if (state !== 'connected' || !shared.listeners.has(listener)) continue
              if (!deliver(listener, event)) {
                fail()
                return
              }
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
          throw new HarnessListError(response.error?.code === METHOD_NOT_FOUND ? 'unsupported' : 'refused')
        })
        .finally(() => {
          clearTimeout(timer)
        })
    },
    subscribe(stream, afterSeq, onEvent, onEnd) {
      if (state !== 'connected') throw unavailable()
      const listener: Listener = { seq: afterSeq, onEvent }
      if (onEnd) listener.onEnd = onEnd
      const existing = streams.get(stream)
      if (existing && covers(existing, afterSeq)) {
        existing.listeners.add(listener)
        for (const { event } of [...existing.events]) {
          if (!existing.listeners.has(listener)) break
          if (!deliver(listener, event)) {
            disconnect()
            break
          }
        }
        return () => existing.listeners.delete(listener)
      }
      if (existing) {
        // Os eventos pedidos já não estão guardados: só este ouvinte termina, como o núcleo faria,
        // mesmo que o núcleo ainda os tenha. Não se pede de novo ao núcleo de propósito: o protocolo
        // não cancela assinaturas, então uma segunda no mesmo stream e conexão ficaria viva para
        // sempre, e o resultado final de cada uma chega pelo id do próprio pedido, o que exigiria
        // ligar vários ids ao mesmo stream no cliente e no daemon. Quem precisa de histórico mais
        // antigo aumenta `replayEvents`/`replayBytes` até `events_per_stream` do daemon.
        let cancelled = false
        queueMicrotask(() => {
          if (!cancelled && streams.get(stream) === existing) listener.onEnd?.(-32004)
        })
        return () => {
          cancelled = true
        }
      }
      const shared: Shared = {
        stream,
        start: afterSeq,
        last: afterSeq,
        events: [],
        bytes: 0,
        listeners: new Set(),
      }
      streams.set(stream, shared)
      shared.listeners.add(listener)
      const command = newCommand('events.subscribe', { stream, after_seq: afterSeq })
      subscriptionRequests.set(command.id, shared)
      void transmit(command).catch(() => undefined)
      // Sair só desliga este ouvinte; a assinatura no núcleo segue para os demais e para quem vier.
      return () => shared.listeners.delete(listener)
    },
  }
}
