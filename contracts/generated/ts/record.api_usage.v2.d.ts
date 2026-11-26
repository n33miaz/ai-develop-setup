/* Gerado por tools/gen-ts.mjs a partir de schemas/record.api_usage.v2.schema.json. Não edite à mão. */

/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Uso de uma chamada de API (com desfecho). O que o adaptador de API informa de uma chamada direta a um provedor: alias do provedor, modelo, tokens, custo estimado em micro-dólares, base do custo e desfecho. Nunca leva texto do pedido nem da resposta, nem chave. O chamador o amplia para o registro usage_event.
 */
export interface RecordApiUsageV2 {
  version: 2;
  ts: CommonTimestampV1;
  provider_alias: CommonIdV1;
  model: string;
  tokens_in: number;
  tokens_out: number;
  cost_estimate_micro_usd: number;
  cost_basis: "reported" | "reserved";
  price_version: string;
  outcome:
    | "succeeded_reported"
    | "succeeded_reserved"
    | "charged_uncertain"
    | "charged_error_status"
    | "charged_bad_response"
    | "charged_guard_replaced";
}
