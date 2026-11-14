/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.pty-host.response.v1.schema.json. Não edite à mão. */

/**
 * Resposta do host de PTY. Resposta JSON-RPC 2.0 do host de PTY: ou result ou error, nunca os dois. O result tem uma das formas do contrato (vazio, saudação, limites do replay, encerramento ou lista de sessões). Erro de replay informa o limite: nada é pulado em silêncio.
 */
export type ContractPtyHostResponseV1 = {
  jsonrpc: "2.0";
  id: number | null;
  result?: Empty | Hello | ReplayBounds | Closed | SessionList;
  error?: {
    code:
      | -32700
      | -32600
      | -32601
      | -32602
      | -32603
      | -32001
      | -32002
      | -32003
      | -32004
      | -32005
      | -32006
      | -32007
      | -32008
      | -32009;
    message: string;
    data?: {
      oldest_seq?: number;
      next_seq?: number;
      limit?: number;
    };
  };
} & {
  [k: string]: unknown;
};
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;

export interface Empty {}
export interface Hello {
  contract: "pty-host/1";
}
export interface ReplayBounds {
  oldest_seq: number;
  next_seq: number;
}
export interface Closed {
  exit_code: number | null;
}
export interface SessionList {
  sessions: {
    session_id: CommonIdV1;
    running: boolean;
    exit_code: number | null;
    oldest_seq: number;
    next_seq: number;
  }[];
}
