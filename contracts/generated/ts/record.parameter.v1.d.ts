/* Gerado por tools/gen-ts.mjs a partir de schemas/record.parameter.v1.schema.json. Não edite à mão. */

/**
 * Parâmetro. Um parâmetro do produto como o núcleo o apresenta: a chave, o tipo, o padrão recomendado, o valor em vigor e a explicação. Todo comportamento com mais de uma escolha razoável é um registro destes. Parâmetro nunca guarda segredo: segredo fica no cofre.
 */
export type RecordParameterV1 = {
  version: 1;
  key: string;
  type: "string" | "integer" | "number" | "boolean" | "string_list";
  default: Value;
  value: Value;
  description: string;
  /**
   * @minItems 2
   */
  choices?: [string, string, ...string[]];
};
export type Value = string | number | boolean | string[];
