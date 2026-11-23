/* Gerado por tools/gen-ts.mjs a partir de schemas/command.harness.list.response.v2.schema.json. Não edite à mão. */

/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Resposta de harness.list. Resposta JSON-RPC 2.0 aceita de harness.list. Lista as ferramentas ligadas e os apelidos de pasta. A versão 2 acrescenta os apelidos das pastas indisponíveis na partida, omitidos quando não há nenhuma. Só nomes; nunca inclui programa, argumentos, ambiente ou caminho.
 */
export interface CommandHarnessListResponseV2 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    harnesses: CommonIdV1[];
    /**
     * Apelidos disponíveis para abrir uma sessão.
     */
    workdir_aliases: CommonIdV1[];
    /**
     * Apelidos cujos caminhos estavam inacessíveis ou não resolviam para uma pasta na partida.
     *
     * @minItems 1
     */
    missing_workdir_aliases?: [CommonIdV1, ...CommonIdV1[]];
  };
}
