/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.local-api.server-message.v8.schema.json. Não edite à mão. */

/**
 * Mensagem do núcleo à interface. Mensagens do núcleo à interface pelo WebSocket da API local. A versão 8 passa a usar a resposta de harness.list que informa os apelidos das pastas ausentes na partida.
 */
export type ContractLocalApiServerMessageV8 =
  | CommandResponseV2
  | CommandHarnessListResponseV2
  | CommandQueueGetResponseV2
  | CommandApprovalListResponseV3
  | EventEnvelopeV1;
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
 * Plano de ação. O plano de uma ação destrutiva sobre arquivos, como o portão o julgou: a operação pedida, os alvos, a raiz de trabalho, a estimativa do ensaio, o veredito com as regras que dispararam, se o apagar virou mover para a quarentena e o destino. O plan_hash cobre o plano em vigor (o reescrito, quando há quarentena) e pode ser reconferido a partir deste registro pela serialização canônica descrita no README. A versão 2 aperta a versão 1 em dois pontos: os caminhos são urn:ads:common:abspath:2 (sem nome de dispositivo nem nome curto do Windows) e a estimativa vai de 0 a 9007199254740991 (2^53 - 1), o maior inteiro que todo leitor do contrato representa sem perda.
 */
export type RecordActionPlanV2 = {
  version: 2;
  action_id: CommonIdV1;
  plan_hash: CommonSha256V1;
  operation: "fs.delete" | "fs.delete_recursive" | "fs.move" | "fs.overwrite" | "fs.clean_untracked";
  work_root: CommonAbspathV2;
  destination: CommonAbspathV2 | null;
  /**
   * @minItems 1
   * @maxItems 10000
   */
  targets: [
    {
      path: CommonAbspathV2;
      pattern: string | null;
      repo_root: boolean;
    },
    ...{
      path: CommonAbspathV2;
      pattern: string | null;
      repo_root: boolean;
    }[]
  ];
  estimate: {
    items: number;
    bytes: number;
  } | null;
  verdict: {
    decision: "allowed" | "requires_approval" | "denied";
    risk: CommonRiskV1;
    /**
     * @maxItems 1000
     */
    reasons: {
      rule: CommonActionRuleV1;
      subject: string;
    }[];
  };
  quarantined: boolean;
};
/**
 * Caminho absoluto normalizado. Caminho absoluto literal, já normalizado, no estilo Windows (unidade em maiúscula e barra invertida) ou POSIX (barra normal): um separador só, sem separador no fim (salvo a raiz), sem componente '.' ou '..', sem variável ($ ou %), sem til no começo de componente, sem curinga e sem caminho de rede. A versão 2 recusa também, no estilo Windows, o que é apelido de outro caminho: componente que é nome de dispositivo (CON, PRN, AUX, NUL, COM0 a COM9, LPT0 a LPT9 e os mesmos com ¹, ² ou ³, com qualquer extensão) e nome curto 8.3 (til seguido de dígitos até o fim do nome ou até um ponto).
 */
export type CommonAbspathV2 = string;
/**
 * Regra do portão de ações. Código estável de uma regra do portão de ações que disparou num plano: palavras minúsculas com sublinhado e, quando a regra tem um detalhe, um ponto e o detalhe (protected.user_home). A lista dos códigos em uso está no README; código novo não muda a versão.
 */
export type CommonActionRuleV1 = string;
/**
 * Classe do dado. Classe de sensibilidade de um dado: public, internal, private ou secret. Segredo nunca sai da máquina.
 */
export type CommonDataClassV1 = "public" | "internal" | "private" | "secret";

/**
 * Resposta de harness.list. Resposta JSON-RPC 2.0 aceita de harness.list. Lista as ferramentas ligadas e os apelidos de pasta. A versão 2 acrescenta os apelidos das pastas ausentes na partida, omitidos quando não há nenhuma. Só nomes; nunca inclui programa, argumentos, ambiente ou caminho.
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
     * Apelidos indisponíveis porque a pasta estava ausente na partida.
     *
     * @minItems 1
     */
    missing_workdir_aliases?: [CommonIdV1, ...CommonIdV1[]];
  };
}
/**
 * Resposta de queue.get. Resposta aceita de queue.get: estado da fila e política vigente registrada no histórico. A política é null se nunca foi registrada.
 */
export interface CommandQueueGetResponseV2 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    queue_id: CommonIdV1;
    state: "stopped" | "enabled";
    policy: CommonQueuePolicyV1 | null;
  };
}
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
/**
 * Resposta de approval.list. Resposta JSON-RPC 2.0 aceita de approval.list: as ações que aguardam a resposta do dono, da mais antiga à mais nova, cada uma com a aprovação pedida, o registro do plano como o portão o julgou (alvos como pedidos, destino, quarentena, estimativa e regras), os efeitos concretos que o ensaio achou (record.action-effects:1, com o hash que a aprovação compromete; null quando não há registro legível), o código da recusa do ensaio (effects_error, ou null) e os motivos pelos quais a API não a deixa aprovar (not_approvable; vazio quando é aprovável). O campo more indica que o limite de itens ou de bytes deixou outra ação listável para depois. O campo unreadable conta os pedidos pendentes com histórico ilegível que a listagem deixou de fora até onde chegou (todos quando more é false); os ids ficam no log do daemon e approval.close os fecha. A versão 2 acrescentou effects, effects_hash, effects_error e more, e os motivos effects_refused, effects_unreadable e effects_not_listed; a versão 3 acrescenta unreadable. A recusa volta como command.response:2 com error.
 */
export interface CommandApprovalListResponseV3 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  result: {
    /**
     * @maxItems 1000
     */
    approvals: {
      action_id: CommonIdV1;
      approval_id: CommonIdV1;
      plan_hash: CommonSha256V1;
      risk: CommonRiskV1;
      summary: string;
      requested_at: CommonTimestampV1;
      expires_at: CommonTimestampV1;
      /**
       * @maxItems 7
       */
      not_approvable:
        | []
        | [
            | "estimate_unknown"
            | "wildcard_target"
            | "definitive_recursive_delete"
            | "restore_from_manifest"
            | "effects_refused"
            | "effects_unreadable"
            | "effects_not_listed"
          ]
        | [
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            )
          ]
        | [
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            )
          ]
        | [
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            )
          ]
        | [
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            )
          ]
        | [
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            )
          ]
        | [
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            ),
            (
              | "estimate_unknown"
              | "wildcard_target"
              | "definitive_recursive_delete"
              | "restore_from_manifest"
              | "effects_refused"
              | "effects_unreadable"
              | "effects_not_listed"
            )
          ];
      plan: RecordActionPlanV2;
      effects_hash: CommonSha256V1 | null;
      effects: RecordActionEffectsV1 | null;
      effects_error: string | null;
    }[];
    /**
     * Indica que o limite de itens ou de bytes deixou outra ação listável para depois.
     */
    more: boolean;
    /**
     * Pedidos pendentes com histórico ilegível deixados fora da listagem até onde ela chegou; com more false, todos os que estão pendentes.
     */
    unreadable: number;
  };
}
/**
 * Efeitos concretos de uma ação. O que o ensaio de uma ação achou no disco e o que a execução vai fazer, alvo por alvo: o caminho, o efeito, onde a quarentena guarda o alvo ou de onde a restauração o tira, e cada entrada da árvore (nomes abaixo do alvo, tipo, tamanho de arquivo e identidade no volume). Na restauração, manifest é o resumo dos bytes do manifesto de onde vieram os pares; um item do manifesto que não está mais na quarentena aparece sem entradas. É o que a aprovação cobre: a execução refaz o ensaio e recusa antes de tocar o disco se achar outro registro. O tamanho é limitado pelo parâmetro max_effects_bytes do executor.
 */
export interface RecordActionEffectsV1 {
  version: 1;
  action_id: CommonIdV1;
  plan_hash: CommonSha256V1;
  manifest: CommonSha256V1 | null;
  /**
   * @minItems 1
   */
  steps: [
    {
      target: CommonAbspathV2;
      effect: "quarantine" | "restore" | "delete" | "delete_recursive";
      stored: CommonAbspathV2 | null;
      entries: {
        /**
         * @maxItems 128
         */
        path: string[];
        kind: "dir" | "file" | "link";
        bytes: number;
        id: string;
      }[];
    },
    ...{
      target: CommonAbspathV2;
      effect: "quarantine" | "restore" | "delete" | "delete_recursive";
      stored: CommonAbspathV2 | null;
      entries: {
        /**
         * @maxItems 128
         */
        path: string[];
        kind: "dir" | "file" | "link";
        bytes: number;
        id: string;
      }[];
    }[]
  ];
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
