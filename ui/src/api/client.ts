import type { CommandAck, CoreCommand, CoreEvent, ConnectionState, Seq } from './types'

export type Unsubscribe = () => void

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
   * a UI viu; 0 para ler desde o começo) e depois os novos, sem repetir nem pular.
   */
  subscribe(stream: string, afterSeq: Seq, onEvent: (event: CoreEvent) => void): Unsubscribe
  /** Envia o comando; repetir a mesma chave não repete o efeito. */
  send(command: CoreCommand, idempotencyKey: string): Promise<CommandAck>
}
