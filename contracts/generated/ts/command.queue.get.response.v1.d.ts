/* Gerado por tools/gen-ts.mjs a partir de schemas/command.queue.get.response.v1.schema.json. Não edite à mão. */

/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Resposta de queue.get. Resposta JSON-RPC 2.0 aceita de queue.get: a fila pedida e o estado dela no núcleo. A recusa volta como command.response:2 com error.
 */
export interface CommandQueueGetResponseV1 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    queue_id: CommonIdV1;
    state: "stopped" | "enabled";
  };
}
