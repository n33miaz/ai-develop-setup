/* Gerado por tools/gen-ts.mjs a partir de schemas/record.host_event.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

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
