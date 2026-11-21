/* Gerado por tools/gen-ts.mjs a partir de schemas/event.closure.recorded.v1.schema.json. Não edite à mão. */

/**
 * Fechamento de pedido ilegível registrado. O dono fechou, pelo caminho de recuperação (approval.close), os pedidos de aprovação pendentes de uma ação cujo histórico o núcleo não lê: o aparelho (actor), o dono, o instante (ts), a decisão (deny ou expire), os pedidos fechados (approval_ids) e a chave de idempotência do comando, que deduplica o fechamento no log. Gravado no stream closure/<action_id> antes de qualquer efeito; cada pedido ganha depois approval.denied:2 ou approval.expired:2 com cause apontando para este registro. Nunca aprova e nunca emite licença.
 */
export type EventClosureRecordedV1 = EventEnvelopeV1 & {
  type: "urn:ads:event:closure.recorded:1";
  stream: string;
  data: {
    action_id: CommonIdV1;
    /**
     * @minItems 1
     * @maxItems 64
     */
    approval_ids: [CommonIdV1, ...CommonIdV1[]];
    decision: "deny" | "expire";
    owner_id: CommonIdV1;
    via: "local_ui";
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
