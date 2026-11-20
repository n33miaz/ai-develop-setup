import type { CommandRequestV1, CommandResponseV2, EventEnvelopeV1 } from './contracts'

export type Unsubscribe = () => void

/** Estado da ligação com o núcleo. É estado do cliente, não mensagem: não vem dos contratos. */
export type ConnectionState = 'disconnected' | 'connecting' | 'connected'

/** O núcleo recusou o replay do fluxo, ou o fluxo foi podado: só esta assinatura termina. */
export const SUBSCRIPTION_REPLAY_UNAVAILABLE = -32004
/** As tentativas automáticas de fechar a sessão se esgotaram: falta um `session.close`. */
export const SESSION_CLOSE_PENDING = -32013
/** A preparação depois de `session.open` estourou o prazo; o núcleo agenda o fechamento. */
export const SESSION_SETUP_TIMED_OUT = -32014

/** Códigos que encerram uma assinatura sem derrubar a conexão. */
export type SubscriptionEndCode =
  typeof SUBSCRIPTION_REPLAY_UNAVAILABLE | typeof SESSION_CLOSE_PENDING

export function isSubscriptionEndCode(code: number): code is SubscriptionEndCode {
  return code === SUBSCRIPTION_REPLAY_UNAVAILABLE || code === SESSION_CLOSE_PENDING
}

/** Erro previsto do cliente (ex.: usar o núcleo sem estar conectado). */
export class CoreClientError extends Error {
  readonly code: 'not_connected'

  constructor(code: 'not_connected', message: string) {
    super(message)
    this.name = 'CoreClientError'
    this.code = code
  }
}

/**
 * Contrato do cliente do núcleo. A UI nunca guarda verdade: só reproduz eventos recebidos.
 * Casca, navegador e celular usam a mesma interface; a implementação real entra depois.
 */
export interface CoreClient {
  getState(): ConnectionState
  onStateChange(listener: (state: ConnectionState) => void): Unsubscribe
  connect(): Promise<void>
  disconnect(): void
  /**
   * Entrega, em ordem de `seq`, os eventos do fluxo com `seq` maior que `afterSeq` (o último que
   * a UI viu; 0 para ler desde o começo) e depois os novos, sem repetir nem pular. Se o núcleo
   * encerrar só esta assinatura (`-32004` ou `-32013`), ela para de entregar eventos, `onEnd`
   * recebe o código e a conexão continua.
   */
  subscribe(
    stream: string,
    afterSeq: number,
    onEvent: (event: EventEnvelopeV1) => void,
    onEnd?: (code: SubscriptionEndCode) => void,
  ): Unsubscribe
  /**
   * Envia o comando. A chave de idempotência vai em `params._meta`; repetir a mesma chave não
   * repete o efeito e a resposta volta com `replayed: true`. Um comando não é um evento: o efeito
   * volta como evento.
   */
  send(command: CommandRequestV1): Promise<CommandResponseV2>
}
