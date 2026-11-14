// Ponto único de importação dos tipos das mensagens trocadas com o núcleo.
// Tipos provisórios e mínimos: serão substituídos pelos gerados dos JSON Schemas de `contracts/`.
// Quando isso acontecer, só este arquivo muda; o resto da UI importa daqui.

/** Número de sequência de um evento dentro do seu fluxo (`stream`), crescente e sem repetição. */
export type Seq = number

/** Evento gravado pelo núcleo: um fato, só-anexo, numerado por fluxo. */
export interface CoreEvent {
  id: string
  type: string
  ts: string
  stream: string
  seq: Seq
  cause?: string
  data: unknown
}

/** Pedido enviado ao núcleo (JSON-RPC 2.0). Um comando não é um evento: o efeito volta como evento. */
export interface CoreCommand {
  method: string
  params?: unknown
}

/** Resposta ao comando: aceito ou repetido (mesma chave de idempotência), nunca aplicado duas vezes. */
export interface CommandAck {
  idempotencyKey: string
  duplicate: boolean
  stream: string
  seq: Seq
}

export type ConnectionState = 'disconnected' | 'connecting' | 'connected'
