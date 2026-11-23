/* Gerado por tools/gen-ts.mjs a partir de schemas/record.action-effects.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;
/**
 * Caminho absoluto normalizado. Caminho absoluto literal, já normalizado, no estilo Windows (unidade em maiúscula e barra invertida) ou POSIX (barra normal): um separador só, sem separador no fim (salvo a raiz), sem componente '.' ou '..', sem variável ($ ou %), sem til no começo de componente, sem curinga e sem caminho de rede. A versão 2 recusa também, no estilo Windows, o que é apelido de outro caminho: componente que é nome de dispositivo (CON, PRN, AUX, NUL, COM0 a COM9, LPT0 a LPT9 e os mesmos com ¹, ² ou ³, com qualquer extensão) e nome curto 8.3 (til seguido de dígitos até o fim do nome ou até um ponto).
 */
export type CommonAbspathV2 = string;

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
