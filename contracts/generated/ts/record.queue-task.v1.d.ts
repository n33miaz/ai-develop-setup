/* Gerado por tools/gen-ts.mjs a partir de schemas/record.queue-task.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Estado da tarefa da fila. Estados do ciclo de uma tarefa da fila contínua.
 */
export type CommonQueueTaskStateV1 = "queued" | "running" | "review" | "done" | "failed" | "cancelled";
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Tarefa da fila. Retrato versionado de uma tarefa. A pasta é referida por apelido; o teto de tentativas inclui a primeira execução.
 */
export interface RecordQueueTaskV1 {
  version: 1;
  id: CommonIdV1;
  queue_id: CommonIdV1;
  origin:
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
  title: string;
  workspace_alias: string;
  tool: CommonIdV1;
  state: CommonQueueTaskStateV1;
  attempts: number;
  attempt_limit: number;
  created_at: CommonTimestampV1;
  updated_at: CommonTimestampV1;
  branch?: string;
  pull_request?: number;
}
