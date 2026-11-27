/* Gerado por tools/gen-ts.mjs a partir de schemas/event.session.handoff_proposed.v2.schema.json. Não edite à mão. */

/**
 * Passagem proposta por limiar de contexto. O núcleo propõe passar o trabalho a uma sessão nova porque o contexto da sessão atingiu o limiar configurado. Só metadados: nunca leva conteúdo da conversa. A decisão do dono é registrada em session.handoff.
 */
export type EventSessionHandoffProposedV2 = EventEnvelopeV1 & {
  type: "urn:ads:event:session.handoff_proposed:2";
  stream: string;
  data_class?: "private";
  data: {
    session_id: CommonIdV1;
    /**
     * Limiar configurado que foi atingido, em percentual da janela de contexto.
     */
    threshold_pct: number;
    /**
     * Percentual medido da janela de contexto ocupada, arredondado para baixo.
     */
    context_pct: number;
    /**
     * Número N da sessão atual na cadeia; a primeira sessão é 1.
     */
    number: number;
    /**
     * Documento de passagem que abriu a sessão atual; nulo na primeira sessão da cadeia.
     */
    previous_document_id: CommonIdV1 | null;
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
