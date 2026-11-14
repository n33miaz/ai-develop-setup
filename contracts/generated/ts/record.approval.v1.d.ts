/* Gerado por tools/gen-ts.mjs a partir de schemas/record.approval.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;
/**
 * Nível de risco. Nível de risco de uma ação ou tarefa: low, medium ou high.
 */
export type CommonRiskV1 = "low" | "medium" | "high";
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Aprovação. Estado de uma aprovação: vale para o hash de um plano e expira. Pendente e expirada não têm decisor; concedida e negada têm autor e instante da decisão.
 */
export interface RecordApprovalV1 {
  version: 1;
  approval_id: CommonIdV1;
  plan_hash: CommonSha256V1;
  risk: CommonRiskV1;
  summary: string;
  status: "pending" | "granted" | "denied" | "expired";
  requested_at: CommonTimestampV1;
  expires_at: CommonTimestampV1;
  decided_at: CommonTimestampV1 | null;
  decided_by: CommonActorV1 | null;
  via: ("local_ui" | "tray" | "remote_device") | null;
  reason: string | null;
}
/**
 * Autor de um fato. Quem causou um evento ou uma decisão: um aparelho, um agente de IA ou o próprio daemon.
 */
export interface CommonActorV1 {
  kind: "device" | "agent" | "daemon";
  id: CommonIdV1;
}
