/* Gerado por tools/gen-ts.mjs a partir de schemas/event.effects.refused.v1.schema.json. Não edite à mão. */

/**
 * Ensaio de efeitos recusado. O ensaio de uma ação que pede aprovação não gerou registro de efeitos: o executor recusou o ensaio (por exemplo E_FS_EFFECTS_TOO_LARGE, acima de max_effects_bytes, ou E_FS_PLAN_ENTRY_LIMIT) ou o núcleo não conseguiu gravar o registro (E_EFFECTS_UNRECORDABLE). Gravado pelo núcleo no lugar de effects.recorded:1, no stream effects/<action_id>, uma vez só (seq 1). A ação continua sem poder ser aprovada pela API onde precisa de efeitos, e a listagem mostra o código.
 */
export type EventEffectsRefusedV1 = EventEnvelopeV1 & {
  type: "urn:ads:event:effects.refused:1";
  stream: string;
  seq: 1;
  data: {
    action_id: CommonIdV1;
    plan_hash: CommonSha256V1;
    error_code: string;
  };
  actor: {
    kind?: "daemon";
    [k: string]: unknown;
  };
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
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;

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
