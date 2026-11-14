/* Gerado por tools/gen-ts.mjs a partir de schemas/event.host.alert_raised.v1.schema.json. Não edite à mão. */

/**
 * Alerta do host emitido. Uma regra de alerta disparou a partir de amostras; o dado é um registro host_event.
 */
export type EventHostAlertRaisedV1 = EventEnvelopeV1 & {
  type: "urn:ads:event:host.alert_raised:1";
  stream: string;
  data: RecordHostEventV1;
};
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
/**
 * Alerta do host. Um alerta derivado de amostras: cita a regra, a gravidade e os sample_id que o justificam. Confirmar é um evento posterior; o alerta original não muda.
 */
export interface RecordHostEventV1 {
  version: 1;
  event_id: CommonIdV1;
  rule_id: CommonIdV1;
  severity: "info" | "warn" | "critical";
  /**
   * @minItems 1
   */
  signals: [CommonIdV1, ...CommonIdV1[]];
  why: string;
  ts: CommonTimestampV1;
  ack: boolean;
  task_id?: CommonIdV1 | null;
}
