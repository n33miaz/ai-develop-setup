/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.local-api.server-message.v3.schema.json. Não edite à mão. */

/**
 * Mensagem do núcleo à interface. Tudo o que o núcleo manda à interface pelo WebSocket da API local: a resposta a um comando (command.response:2), a resposta aceita de harness.list (command.harness.list.response:1), a resposta aceita de queue.get (command.queue.get.response:1) ou um evento de um stream assinado (event.envelope:1). Cada mensagem é uma das quatro, nunca mais de uma. A versão 3 acrescenta à :2 a resposta de queue.get.
 */
export type ContractLocalApiServerMessageV3 =
  CommandResponseV2 | CommandHarnessListResponseV1 | CommandQueueGetResponseV1 | EventEnvelopeV1;
/**
 * Resposta de comando. Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois. A versão 2 fecha a forma do result: o comando foi aceito, e replayed diz se esta é a repetição de um comando já aceito com a mesma chave de idempotência (nada foi executado de novo). O efeito do comando volta como evento, não no result.
 */
export type CommandResponseV2 = {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result?: {
    accepted: true;
    replayed: boolean;
  };
  error?: {
    code: number;
    message: string;
    data?: {};
  };
} & {
  [k: string]: unknown;
};
/**
 * Identificador ULID. Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo.
 */
export type CommonUlidV1 = string;
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;
/**
 * Classe do dado. Classe de sensibilidade de um dado: public, internal, private ou secret. Segredo nunca sai da máquina.
 */
export type CommonDataClassV1 = "public" | "internal" | "private" | "secret";

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
/**
 * Resposta de queue.get. Resposta JSON-RPC 2.0 aceita de queue.get: a fila pedida e o estado dela no núcleo. A recusa volta como command.response:2 com error.
 */
export interface CommandQueueGetResponseV1 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    queue_id: CommonIdV1;
    state: "stopped" | "enabled";
  };
}
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
