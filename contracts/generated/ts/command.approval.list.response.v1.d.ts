/* Gerado por tools/gen-ts.mjs a partir de schemas/command.approval.list.response.v1.schema.json. Não edite à mão. */

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
 * Resposta de approval.list. Resposta JSON-RPC 2.0 aceita de approval.list: as ações que aguardam a resposta do dono, da mais antiga à mais nova, cada uma com a aprovação pedida, o registro do plano como o portão o julgou (alvos como pedidos, destino, quarentena, estimativa e regras) e os motivos pelos quais a API não a deixa aprovar (not_approvable; vazio quando é aprovável). O registro não lista o conteúdo das pastas nem os itens de um manifesto: esses são lidos na execução. A recusa volta como command.response:2 com error.
 */
export interface CommandApprovalListResponseV1 {
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
       * @maxItems 4
       */
      not_approvable:
        | []
        | ["estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest"]
        | [
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest",
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest"
          ]
        | [
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest",
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest",
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest"
          ]
        | [
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest",
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest",
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest",
            "estimate_unknown" | "wildcard_target" | "definitive_recursive_delete" | "restore_from_manifest"
          ];
      plan: RecordActionPlanV2;
    }[];
  };
}
