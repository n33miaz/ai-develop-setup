/* Gerado por tools/gen-ts.mjs a partir de schemas/record.session-handoff.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Passagem entre sessões. Passagem autorizada para uma nova sessão, com referências opacas aos artefatos mantidos pelo produto.
 */
export interface RecordSessionHandoffV1 {
  id: CommonIdV1;
  source_session_id: CommonIdV1;
  target_session_id: CommonIdV1;
  previous_handoff_id: CommonIdV1 | null;
  trigger: "context_limit" | "account_switch" | "model_refusal";
  lineage: CommonIdV1;
  number: number;
  created_at: CommonTimestampV1;
  state_document_id: CommonIdV1;
  opening_prompt_id: CommonIdV1;
  consent: {
    user_id: CommonIdV1;
    channel: "local" | "remote";
  };
}
