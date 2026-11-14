/* Gerado por tools/gen-ts.mjs a partir de schemas/event.usage.recorded.v1.schema.json. Não edite à mão. */

/**
 * Uso registrado. Uma chamada ou turno de IA foi medido: o dado é um registro usage_event completo, gravado só-anexo no banco de métricas.
 */
export type EventUsageRecordedV1 = EventEnvelopeV1 & {
  type: "urn:ads:event:usage.recorded:1";
  stream: string;
  data: RecordUsageEventV1;
};
/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Classe do dado. Classe de sensibilidade de um dado: public, internal, private ou secret. Segredo nunca sai da máquina.
 */
export type CommonDataClassV1 = "public" | "internal" | "private" | "secret";

/**
 * Envelope de evento. Envelope único de todo evento: fato gravado só-anexo, numerado por stream, com autor, causa e classe do dado. Cada tipo de evento estende este envelope e fixa type, stream e data.
 */
export interface EventEnvelopeV1 {
  id: CommonUlidV1;
  type: string;
  ts: CommonTimestampV1;
  stream: string;
  seq: number;
  actor: CommonActorV1;
  cause: CommonUlidV1 | null;
  data_class: CommonDataClassV1;
  data: {};
}
/**
 * Autor de um fato. Quem causou um evento ou uma decisão: um aparelho, um agente de IA ou o próprio daemon.
 */
export interface CommonActorV1 {
  kind: "device" | "agent" | "daemon";
  id: CommonIdV1;
}
/**
 * Evento de uso. Uma linha do livro-caixa: uma chamada ou turno de IA medido, com tokens, duração, custo de lista, custo faturável (só chave de API), janela de cota, fonte e chave de deduplicação. Só-anexo; alias de conta nunca é credencial nem e-mail.
 */
export interface RecordUsageEventV1 {
  version: 1;
  event_id: CommonIdV1;
  ts: CommonTimestampV1;
  task_id: CommonIdV1 | null;
  parent_task_id: CommonIdV1 | null;
  session_id: CommonIdV1 | null;
  workspace: CommonIdV1;
  project_id: CommonIdV1 | null;
  os_user: CommonIdV1;
  role: "executor" | "reviewer" | "router" | "advisor" | "orchestrator";
  harness: CommonIdV1;
  provider: CommonIdV1;
  account_alias: CommonIdV1;
  model_requested: string | null;
  model_effective: string;
  effort_requested: string | null;
  effort_effective: string | null;
  routing_mode: ("manual" | "propose" | "auto" | "shadow") | null;
  routing_rule_id: CommonIdV1 | null;
  tokens_in: number;
  tokens_out: number;
  tokens_cache_read: number;
  tokens_cache_write: number;
  duration_ms: number;
  status: "ok" | "error" | "429" | "aborted" | "fallback";
  error_code: string | null;
  price_version: string;
  cost_list_usd: number | null;
  cost_billable_usd: number | null;
  quota_window_before: {
    five_hour_pct: number | null;
    weekly_pct: number | null;
  } | null;
  quota_window_after: {
    five_hour_pct: number | null;
    weekly_pct: number | null;
  } | null;
  source: "adapter" | "cli_json" | "statusline" | "transcript" | "rollout" | "usagebar" | "manual";
  source_ref: string;
  dedupe_key: string;
}
