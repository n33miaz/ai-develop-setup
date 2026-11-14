/* Gerado por tools/gen-ts.mjs a partir de schemas/record.host_sample.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Amostra do host. Uma observação de um sinal da máquina (disco, memória, etc.) feita por um coletor só leitura. Amostra ausente nunca vira zero; corrigir é gravar outra amostra com outro sample_id.
 */
export interface RecordHostSampleV1 {
  version: 1;
  sample_id: CommonIdV1;
  signal_id: string;
  value: number | boolean | string;
  unit: string;
  ts: CommonTimestampV1;
  collector: CommonIdV1;
  cost_ms: number;
  scope_alias: CommonIdV1;
  stale: boolean;
  task_id?: CommonIdV1 | null;
}
