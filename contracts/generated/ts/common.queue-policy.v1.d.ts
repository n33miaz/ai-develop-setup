/* Gerado por tools/gen-ts.mjs a partir de schemas/common.queue-policy.v1.schema.json. Não edite à mão. */

/**
 * Limites da fila. Limites de admissão e prazo de confirmação da sessão pelo host.
 */
export interface CommonQueuePolicyV1 {
  attempt_limit: number;
  admission_limit: number;
  window_seconds: number;
  window_admissions: number;
  manual_window_admissions: number;
  bug_rule_window_admissions: number;
  clock_skew_seconds: number;
  clock_rewind_seconds: number;
  host_confirm_seconds: number;
}
