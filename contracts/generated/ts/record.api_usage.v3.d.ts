/* Gerado por tools/gen-ts.mjs a partir de schemas/record.api_usage.v3.schema.json. Não edite à mão. */

/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
export type SafeInteger = number;

/**
 * Uso de uma chamada de API (com desfecho). O que o adaptador de API informa de uma chamada direta a um provedor: alias do provedor, modelo, tokens, custo estimado em micro-dólares, base do custo e desfecho. Nunca leva texto do pedido nem da resposta, nem chave. O chamador o amplia para o registro usage_event. As contagens e o custo usam inteiros de 0 a 9007199254740991 (2^53 - 1), representáveis exatamente em Rust e TypeScript.
 */
export interface RecordApiUsageV3 {
  version: 3;
  ts: CommonTimestampV1;
  provider_alias: CommonIdV1;
  model: string;
  tokens_in: SafeInteger;
  tokens_out: SafeInteger;
  cost_estimate_micro_usd: SafeInteger;
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
