/* Gerado por tools/gen-ts.mjs a partir de schemas/command.harness.list.response.v1.schema.json. Não edite à mão. */

/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Resposta de harness.list. Resposta JSON-RPC 2.0 aceita de harness.list: os identificadores das ferramentas ligadas na configuração do usuário e os apelidos de pasta de trabalho, só nomes. Nunca leva programa, argumentos, ambiente nem caminho. A recusa volta como command.response:2 com error.
 */
export interface CommandHarnessListResponseV1 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    harnesses: CommonIdV1[];
    workdir_aliases: CommonIdV1[];
  };
}
