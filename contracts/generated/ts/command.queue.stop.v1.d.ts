/* Gerado por tools/gen-ts.mjs a partir de schemas/command.queue.stop.v1.schema.json. Não edite à mão. */

/**
 * Parar a fila. O dono para a fila. Parar expressa o estado desejado e é sempre aceito: o núcleo não consulta a cache de idempotência, então repetir um queue.stop antigo depois de religar a fila a para de novo, e uma chave já usada por outro comando não impede a parada. A resposta é command.response:2 com replayed true quando a fila já estava parada (nada foi gravado) e false quando esta parada a desligou e anexou queue.stopped com reason owner. A fila para antes da gravação; se a gravação falhar, a resposta é INTERNAL_ERROR com a fila parada, e o evento é gravado antes da próxima mudança feita pelo mesmo processo, com o id deste comando em cause.
 */
export type CommandQueueStopV1 = CommandRequestV1 & {
  method: "queue.stop";
  params: {
    queue_id: CommonIdV1;
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
