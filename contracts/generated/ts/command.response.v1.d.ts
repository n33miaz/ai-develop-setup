/* Gerado por tools/gen-ts.mjs a partir de schemas/command.response.v1.schema.json. Não edite à mão. */

/**
 * Resposta de comando. Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois.
 */
export type CommandResponseV1 = {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result?: {};
  error?: {
    code: number;
    message: string;
    data?: {};
  };
} & {
  [k: string]: unknown;
};
/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
