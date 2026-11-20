/* Gerado por tools/gen-ts.mjs a partir de schemas/record.task-queue.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Fila contínua. Controle da fila. Toda fila nasce parada; só o dono autenticado pode ativá-la.
 */
export interface RecordTaskQueueV1 {
  version: 1;
  id: CommonIdV1;
  owner_id: CommonIdV1;
  state: "stopped" | "enabled";
}
