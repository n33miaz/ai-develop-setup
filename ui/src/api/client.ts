import {
  SESSION_CLOSE_PENDING,
  SESSION_SETUP_FAILED,
  SUBSCRIPTION_REPLAY_UNAVAILABLE,
  type CommandRequestV1,
  type CommandQueueConfiguredResponseV1,
  type CommandQueueGetResponseV2,
  type CommandApprovalListResponseV3,
  type CommandResponseV2,
  type EventEnvelopeV1,
  type HarnessCatalog,
} from './contracts'

export type LocalCommandReply =
  | CommandResponseV2
  | ((
      CommandQueueGetResponseV2 | CommandQueueConfiguredResponseV1 | CommandApprovalListResponseV3
    ) & { error?: never })

export type Unsubscribe = () => void

/** Estado da ligação com o núcleo. É estado do cliente, não mensagem: não vem dos contratos. */
export type ConnectionState =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'proof_missing'
  | 'relaunch_required'
  | 'retry_exhausted'

export { SESSION_CLOSE_PENDING, SESSION_SETUP_FAILED, SUBSCRIPTION_REPLAY_UNAVAILABLE }

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

/** Por que `harness.list` falhou: sem resposta no prazo, núcleo que não conhece o método, recusa. */
export type HarnessListFailure = 'timeout' | 'unsupported' | 'refused'

export class HarnessListError extends Error {
  readonly reason: HarnessListFailure

  constructor(reason: HarnessListFailure) {
    super(reason === 'timeout' ? 'core timeout' : 'command rejected')
    this.name = 'HarnessListError'
    this.reason = reason
  }
}

/**
 * Contrato do cliente do núcleo. A UI nunca guarda verdade: só reproduz eventos recebidos.
 * Casca, navegador e celular usam a mesma interface; a implementação real entra depois.
 */
export interface CoreClient {
  getState(): ConnectionState
  /** Muda a cada conexão autenticada; o que veio de uma conexão não vale para a seguinte. */
  getGeneration(): number
  onStateChange(listener: (state: ConnectionState) => void): Unsubscribe
  connect(): Promise<void>
  disconnect(): void
  /**
   * Entrega, em ordem de `seq`, os eventos do fluxo com `seq` maior que `afterSeq` (o último que
   * a UI viu; 0 para ler desde o começo) e depois os novos, sem repetir nem pular. Se o núcleo
   * encerrar só esta assinatura (replay indisponível ou fechamento pendente), ela para de entregar eventos, `onEnd`
   * recebe o código e a conexão continua. Eventos já disponíveis podem chegar a `onEvent` antes de
   * `subscribe` retornar; `onEnd` nunca é chamado dentro dele.
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
  send(command: CommandRequestV1): Promise<LocalCommandReply>
  /** Ferramentas ligadas e apelidos de pasta do núcleo (`harness.list`). A recusa vira erro. */
  listHarnesses(): Promise<HarnessCatalog>
}
