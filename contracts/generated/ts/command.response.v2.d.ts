/* Gerado por tools/gen-ts.mjs a partir de schemas/command.response.v2.schema.json. Não edite à mão. */

/**
 * Resposta de comando. Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois. A versão 2 fecha a forma do result: o comando foi aceito, e replayed diz se esta é a repetição de um comando já aceito com a mesma chave de idempotência (nada foi executado de novo). O efeito do comando volta como evento, não no result.
 */
export type CommandResponseV2 = {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result?: {
    accepted: true;
    replayed: boolean;
  };
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
