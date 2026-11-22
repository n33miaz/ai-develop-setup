/* Gerado por tools/gen-ts.mjs a partir de schemas/command.queue.configured.v1.schema.json. Não edite à mão. */

/**
 * Informar a fila configurada. Pede ao núcleo o id da fila de tarefas que a configuração local dele serve, criada ou não. Só leitura: não muda nada no núcleo e não entra na cache de idempotência. A resposta aceita é command.queue.configured.response:1; sem controles de fila, a recusa volta como command.response:2 com QUEUE_NOT_CONFIGURED.
 */
export type CommandQueueConfiguredV1 = CommandRequestV1 & {
  method: "queue.configured";
  params: {
    _meta: unknown;
  };
};
/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Envelope de comando. Comando JSON-RPC 2.0 enviado ao núcleo. Comando é pedido, não fato; a chave de idempotência evita executar duas vezes, e a origem remota exige assinatura do aparelho, nonce, contador e validade.
 */
export interface CommandRequestV1 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  method: string;
  params: {
    _meta: {
      idempotency_key: CommonUlidV1;
      origin: "local" | "remote";
      remote?: {
        device_id: CommonIdV1;
        signature: string;
        nonce: string;
        counter: number;
        expires_at: CommonTimestampV1;
      };
    };
    [k: string]: unknown;
  };
}
