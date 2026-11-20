/* Gerado por tools/gen-ts.mjs a partir de schemas/event.session.refusal_detected.v1.schema.json. Não edite à mão. */

/**
 * Recusa ou filtro detectado. A ferramenta recusou um pedido ou um filtro de segurança interrompeu a sessão. Só metadados: ferramenta, provedor, identificador do modelo no catálogo e tipo. O texto do pedido e da resposta não cabe no evento.
 */
export type EventSessionRefusalDetectedV1 = EventEnvelopeV1 & {
  type: "urn:ads:event:session.refusal_detected:1";
  stream: string;
  data_class?: "private";
  data: {
    session_id: CommonIdV1;
    harness: CommonIdV1;
    provider: CommonIdV1;
    /**
     * Identificador de um modelo do catálogo configurado: 1 a 48 caracteres de minúsculas, dígitos e . _ -, começando por minúscula ou dígito. Não é texto livre.
     */
    model: string;
    kind: "refusal" | "safety_filter";
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
