/* Gerado por tools/gen-ts.mjs a partir de schemas/record.update-status.v1.schema.json. Não edite à mão. */

/**
 * Estado da atualização. Estado visível da auto-atualização, escrito pelo lançador: o que está rodando, a versão anterior guardada, a candidata em andamento, a prova depois da troca, as quedas contadas e as versões bloqueadas. Baixando, conferindo e trocando têm candidata; em prova há prazo; falha traz código de erro.
 */
export type RecordUpdateStatusV1 = {
  version: 1;
  state: "idle" | "downloading" | "verifying" | "switching" | "probation" | "rolled_back" | "failed";
  current: BuildRef;
  previous: BuildRef | null;
  candidate: BuildRef | null;
  probation_until: CommonTimestampV1 | null;
  crashes: number;
  blocked: string[];
  last_error: string | null;
  updated_at: CommonTimestampV1;
};
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

export interface BuildRef {
  build: string;
  commit: string;
}
