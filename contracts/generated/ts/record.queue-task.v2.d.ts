/* Gerado por tools/gen-ts.mjs a partir de schemas/record.queue-task.v2.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Origem de uma tarefa da fila. De onde veio a tarefa: pedido manual (id estável do pedido), issue (número no repositório do apelido) ou regra de bug (par regra e alvo estável).
 */
export type CommonQueueTaskOriginV1 =
  | {
      kind: "manual";
      request_id: CommonIdV1;
    }
  | {
      kind: "issue";
      ref: number;
    }
  | {
      kind: "bug_rule";
      rule_id: CommonIdV1;
      target_id: CommonIdV1;
    };
/**
 * Apelido de pasta de trabalho. Nome cadastrado de uma pasta de trabalho, em vez do caminho: minúsculas, dígitos e . _ -, de 1 a 64 caracteres, sem '..', sem terminar em ponto e sem nome de dispositivo do Windows.
 */
export type CommonWorkdirAliasV1 = string;
/**
 * Estado da tarefa da fila. Estados do ciclo de uma tarefa da fila contínua.
 */
export type CommonQueueTaskStateV1 = "queued" | "running" | "review" | "done" | "failed" | "cancelled";
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Tarefa da fila. Retrato versionado de uma tarefa. A pasta é referida por apelido; o teto de tentativas inclui a primeira execução. O apelido e a origem são os tipos comuns.
 */
export interface RecordQueueTaskV2 {
  version: 2;
  id: CommonIdV1;
  queue_id: CommonIdV1;
  origin: CommonQueueTaskOriginV1;
  title: string;
  workspace_alias: CommonWorkdirAliasV1;
  tool: CommonIdV1;
  state: CommonQueueTaskStateV1;
  attempts: number;
  attempt_limit: number;
  created_at: CommonTimestampV1;
  updated_at: CommonTimestampV1;
  branch?: string;
  pull_request?: number;
}
