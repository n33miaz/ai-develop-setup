/* Gerado por tools/gen-ts.mjs a partir de schemas/event.answer.recorded.v2.schema.json. Não edite à mão. */

/**
 * Resposta do dono registrada. A resposta do dono a uma aprovação, gravada antes de o portão agir sobre ela: o aparelho (actor), o dono, o instante (ts), a decisão, o plano que o dono viu, o hash do registro de efeitos concretos que a listagem mostrou (effects_hash, null quando a ação não tem esse registro) e a chave de idempotência do comando, que deduplica a resposta no log. Fica no stream answer/<approval_id>. A decisão do portão vem depois, em approval.granted:2 ou approval.denied:2, e pode ser uma negação mesmo quando o dono aprovou (política mudou). A versão 2 acrescenta effects_hash, obrigatório.
 */
export type EventAnswerRecordedV2 = EventEnvelopeV1 & {
  type: "urn:ads:event:answer.recorded:2";
  stream: string;
  data: {
    approval_id: CommonIdV1;
    action_id: CommonIdV1;
    plan_hash: CommonSha256V1;
    effects_hash: CommonSha256V1 | null;
    decision: "approve" | "deny";
    owner_id: CommonIdV1;
    via: "local_ui" | "tray" | "remote_device";
    idempotency_key: CommonUlidV1;
  };
  actor: {
    kind?: "device";
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
