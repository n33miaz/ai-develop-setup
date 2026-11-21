/* Gerado por tools/gen-ts.mjs a partir de schemas/event.effects.recorded.v1.schema.json. Não edite à mão. */

/**
 * Efeitos concretos registrados. O registro record.action-effects:1 que o ensaio de uma ação achou, gravado pelo núcleo logo depois do plano, no stream effects/<action_id>, uma vez só (seq 1). effects_hash é o SHA-256 da serialização do registro que o núcleo grava; a resposta do dono (answer.recorded:2) repete esse hash, e a execução só começa se o ensaio dela achar o mesmo registro.
 */
export type EventEffectsRecordedV1 = EventEnvelopeV1 & {
  type: "urn:ads:event:effects.recorded:1";
  stream: string;
  seq: 1;
  data: {
    action_id: CommonIdV1;
    plan_hash: CommonSha256V1;
    effects_hash: CommonSha256V1;
    effects: RecordActionEffectsV1;
  };
  actor: {
    kind?: "daemon";
    [k: string]: unknown;
  };
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
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;
/**
 * Caminho absoluto normalizado. Caminho absoluto literal, já normalizado, no estilo Windows (unidade em maiúscula e barra invertida) ou POSIX (barra normal): um separador só, sem separador no fim (salvo a raiz), sem componente '.' ou '..', sem variável ($ ou %), sem til no começo de componente, sem curinga e sem caminho de rede. A versão 2 recusa também, no estilo Windows, o que é apelido de outro caminho: componente que é nome de dispositivo (CON, PRN, AUX, NUL, COM0 a COM9, LPT0 a LPT9 e os mesmos com ¹, ² ou ³, com qualquer extensão) e nome curto 8.3 (til seguido de dígitos até o fim do nome ou até um ponto).
 */
export type CommonAbspathV2 = string;

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
