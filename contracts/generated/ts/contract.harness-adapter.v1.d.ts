/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.harness-adapter.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Descritor do adaptador de IA. O que um adaptador declara ao núcleo no contrato harness-adapter/1: qual ferramenta adapta, se é linha de comando ou API, em que modos e papéis atende e quem guarda a credencial. Adaptador de linha de comando nunca toca na credencial que a ferramenta administra; adaptador de API usa a chave do cofre do produto e não tem modo interativo.
 */
export interface ContractHarnessAdapterV1 {
  contract: "harness-adapter/1";
  harness: CommonIdV1;
  kind: "cli" | "api";
  /**
   * @minItems 1
   */
  modes: ["interactive" | "headless", ...("interactive" | "headless")[]];
  /**
   * @minItems 1
   */
  roles: [
    "executor" | "reviewer" | "router" | "advisor" | "orchestrator",
    ...("executor" | "reviewer" | "router" | "advisor" | "orchestrator")[]
  ];
  credential: "cli_managed" | "vault";
}
