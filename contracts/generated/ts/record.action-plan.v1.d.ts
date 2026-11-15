/* Gerado por tools/gen-ts.mjs a partir de schemas/record.action-plan.v1.schema.json. Não edite à mão. */

/**
 * Plano de ação. O plano de uma ação destrutiva sobre arquivos, como o portão o julgou: a operação pedida, os alvos, a raiz de trabalho, a estimativa do ensaio, o veredito com as regras que dispararam, se o apagar virou mover para a quarentena e o destino. O plan_hash cobre o plano em vigor (o reescrito, quando há quarentena) e pode ser reconferido a partir deste registro pela serialização canônica descrita no README.
 */
export type RecordActionPlanV1 = {
  version: 1;
  action_id: CommonIdV1;
  plan_hash: CommonSha256V1;
  operation: "fs.delete" | "fs.delete_recursive" | "fs.move" | "fs.overwrite" | "fs.clean_untracked";
  work_root: CommonAbspathV1;
  destination: CommonAbspathV1 | null;
  /**
   * @minItems 1
   * @maxItems 10000
   */
  targets: [
    {
      path: CommonAbspathV1;
      pattern: string | null;
      repo_root: boolean;
    },
    ...{
      path: CommonAbspathV1;
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
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;
/**
 * Caminho absoluto normalizado. Caminho absoluto literal, já normalizado, no estilo Windows (unidade em maiúscula e barra invertida) ou POSIX (barra normal): um separador só, sem separador no fim (salvo a raiz), sem componente '.' ou '..', sem variável ($ ou %), sem til no começo de componente, sem curinga e sem caminho de rede.
 */
export type CommonAbspathV1 = string;
/**
 * Nível de risco. Nível de risco de uma ação ou tarefa: low, medium ou high.
 */
export type CommonRiskV1 = "low" | "medium" | "high";
/**
 * Regra do portão de ações. Código estável de uma regra do portão de ações que disparou num plano: palavras minúsculas com sublinhado e, quando a regra tem um detalhe, um ponto e o detalhe (protected.user_home). A lista dos códigos em uso está no README; código novo não muda a versão.
 */
export type CommonActionRuleV1 = string;
