/* Gerado por tools/gen-ts.mjs a partir de schemas/command.queue.get.response.v2.schema.json. Não edite à mão. */

/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Resposta de queue.get. Resposta aceita de queue.get: estado da fila e política vigente registrada no histórico. A política é null se nunca foi registrada.
 */
export interface CommandQueueGetResponseV2 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    queue_id: CommonIdV1;
    state: "stopped" | "enabled";
    policy: CommonQueuePolicyV1 | null;
  };
}
/**
 * Limites da fila. Limites de admissão e prazo de confirmação da sessão pelo host.
 */
export interface CommonQueuePolicyV1 {
  attempt_limit: number;
  admission_limit: number;
  window_seconds: number;
  window_admissions: number;
  manual_window_admissions: number;
  bug_rule_window_admissions: number;
  clock_skew_seconds: number;
  clock_rewind_seconds: number;
  host_confirm_seconds: number;
}
