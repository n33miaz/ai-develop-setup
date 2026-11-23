/* Gerado por tools/gen-ts.mjs a partir de schemas/event.queue.task_created.v4.schema.json. Não edite à mão. */

/**
 * Tarefa incluída na fila. Registra a inclusão de uma tarefa em queued, com zero tentativas, junto com o título e a ferramenta que o registro da tarefa mostra. Uma criação de origem manual exige um aparelho como autor. O evento leva texto escrito pelo usuário e por isso nunca é public.
 */
export type EventQueueTaskCreatedV4 = EventEnvelopeV1 & {
  type: "urn:ads:event:queue.task_created:4";
  stream: string;
  data: {
    queue_id: CommonIdV1;
    task_id: CommonIdV1;
    attempt_limit: number;
    origin: CommonQueueTaskOriginV1;
    workspace_alias: CommonWorkdirAliasV1;
    title: CommonQueueTaskTitleV1;
    tool: CommonIdV1;
    state: "queued";
    attempts: 0;
  };
  data_class?: "internal" | "private" | "secret";
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
 * Origem de uma tarefa da fila. De onde veio a tarefa: pedido manual (id estável do pedido), issue (número no repositório do apelido) ou regra de bug (par regra e alvo estável).
 */
export type CommonQueueTaskOriginV1 =
  | {
      kind: "manual";
      request_id: CommonIdV1;
    }
  | {
      kind: "issue";
      ref: number;
    }
  | {
      kind: "bug_rule";
      rule_id: CommonIdV1;
      target_id: CommonIdV1;
    };
/**
 * Apelido de pasta de trabalho. Nome cadastrado de uma pasta de trabalho, em vez do caminho: minúsculas, dígitos e . _ -, de 1 a 64 caracteres, sem '..', sem terminar em ponto e sem nome de dispositivo do Windows.
 */
export type CommonWorkdirAliasV1 = string;
/**
 * Título de uma tarefa da fila. Rótulo curto da tarefa, de 1 a 120 caracteres, no alfabeto fixo do Unicode 16 (README, fila contínua): ao menos uma letra ou um número, sem espaço no início, no fim ou repetido e no máximo três marcas combinantes seguidas.
 */
export type CommonQueueTaskTitleV1 = string;

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
