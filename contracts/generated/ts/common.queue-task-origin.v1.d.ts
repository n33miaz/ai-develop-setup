/* Gerado por tools/gen-ts.mjs a partir de schemas/common.queue-task-origin.v1.schema.json. Não edite à mão. */

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
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
