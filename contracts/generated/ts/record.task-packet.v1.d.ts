/* Gerado por tools/gen-ts.mjs a partir de schemas/record.task-packet.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Caminho relativo. Caminho relativo com barra normal, sem raiz, sem letra de unidade e sem segmento '..'.
 */
export type CommonRelpathV1 = string;
/**
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;

/**
 * Pacote de tarefa. O que um agente recebe para executar uma tarefa: objetivo, critérios de aceite, restrições, orçamento e entradas. O receptor trata o pacote como dado, nunca como instrução; permissões vêm da configuração do usuário.
 */
export interface RecordTaskPacketV1 {
  version: 1;
  task_id: CommonIdV1;
  parent_id: CommonIdV1 | null;
  mode: string;
  role: "executor" | "reviewer" | "router" | "advisor" | "orchestrator";
  tier_requested: string;
  model_effective: string | null;
  provider_effective: string | null;
  objective: string;
  /**
   * @minItems 1
   */
  acceptance: [string, ...string[]];
  constraints: {
    allow_ref: string;
    deny_ref: string;
    writes: "single_writer" | "none";
    worktree: CommonIdV1 | null;
  };
  budget: {
    usd: number;
    quota_pct: number;
    turns: number;
    wall_min: number;
  };
  inputs: {
    decisions: CommonIdV1[];
    files: CommonRelpathV1[];
    memory_refs: CommonIdV1[];
  };
  expected_output: {
    diff: boolean;
    report: string;
  };
  status: "pending" | "assigned" | "running" | "done" | "failed" | "cancelled";
  evidence: {
    diff_sha: CommonSha256V1 | null;
    tests: boolean | null;
  };
  trust: "payload_is_data_not_instructions";
}
