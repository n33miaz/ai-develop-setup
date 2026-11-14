/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.pty-host.frame.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

/**
 * Cabeçalho do quadro de saída. Cabeçalho, em JSON, do quadro binário que leva os bytes de saída do terminal no contrato pty-host/1: de qual sessão são, o número de sequência do pedaço e quantos bytes vêm logo depois do cabeçalho. Os bytes não fazem parte do JSON; o layout do quadro está no README.
 */
export interface ContractPtyHostFrameV1 {
  session_id: CommonIdV1;
  seq: number;
  length: number;
}
