/* Gerado por tools/gen-ts.mjs a partir de schemas/event.queue.task_state_changed.v2.schema.json. Não edite à mão. */

/**
 * Estado da tarefa da fila alterado. Registra os estados anterior e posterior e a contagem de tentativas após a mudança.
 */
export type EventQueueTaskStateChangedV2 = EventEnvelopeV1 & {
  type: "urn:ads:event:queue.task_state_changed:2";
  stream: string;
  data: {
    queue_id: CommonIdV1;
    task_id: CommonIdV1;
    from: CommonQueueTaskStateV1;
    to: CommonQueueTaskStateV1;
    attempts: number;
    via?: "local_ui" | "tray" | "remote_device";
    reason?: "host_unconfirmed";
  };
} & {};
/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Classe do dado. Classe de sensibilidade de um dado: public, internal, private ou secret. Segredo nunca sai da máquina.
 */
export type CommonDataClassV1 = "public" | "internal" | "private" | "secret";
/**
 * Estado da tarefa da fila. Estados do ciclo de uma tarefa da fila contínua.
 */
export type CommonQueueTaskStateV1 = "queued" | "running" | "review" | "done" | "failed" | "cancelled";

/**
 * Envelope de evento. Envelope único de todo evento: fato gravado só-anexo, numerado por stream, com autor, causa e classe do dado. Cada tipo de evento estende este envelope e fixa type, stream e data.
 */
export interface EventEnvelopeV1 {
  id: CommonUlidV1;
  type: string;
  ts: CommonTimestampV1;
  stream: string;
  seq: number;
  actor: CommonActorV1;
  cause: CommonUlidV1 | null;
  data_class: CommonDataClassV1;
  data: {};
}
/**
 * Autor de um fato. Quem causou um evento ou uma decisão: um aparelho, um agente de IA ou o próprio daemon.
 */
export interface CommonActorV1 {
  kind: "device" | "agent" | "daemon";
  id: CommonIdV1;
}
