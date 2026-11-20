/* Gerado por tools/gen-ts.mjs a partir de schemas/common.queue-task-state.v1.schema.json. Não edite à mão. */

/**
 * Estado da tarefa da fila. Estados do ciclo de uma tarefa da fila contínua.
 */
export type CommonQueueTaskStateV1 = "queued" | "running" | "review" | "done" | "failed" | "cancelled";
