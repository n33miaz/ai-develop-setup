/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.pty-host.request.v1.schema.json. Não edite à mão. */

/**
 * Pedido ao host de PTY. Pedido JSON-RPC 2.0 do núcleo ao host de PTY no contrato pty-host/1, com envelope próprio: id inteiro por conexão, sem chave de idempotência nem assinatura, porque o canal é local e do próprio usuário. Os bytes de saída do terminal não vêm em resposta: viajam no quadro binário.
 */
export type ContractPtyHostRequestV1 =
  | {
      jsonrpc: "2.0";
      id: number;
      method: "host.hello";
      params: {
        contract: "pty-host/1";
      };
    }
  | {
      jsonrpc: "2.0";
      id: number;
      method: "session.open";
      params: {
        session_id: CommonIdV1;
        program: string;
        args: string[];
        cwd: string | null;
        env: {
          [k: string]: string;
        };
        cols: number;
        rows: number;
      };
    }
  | {
      jsonrpc: "2.0";
      id: number;
      method: "session.write";
      params: {
        session_id: CommonIdV1;
        data: CommonBase64V1;
      };
    }
  | {
      jsonrpc: "2.0";
      id: number;
      method: "session.resize";
      params: {
        session_id: CommonIdV1;
        cols: number;
        rows: number;
      };
    }
  | {
      jsonrpc: "2.0";
      id: number;
      method: "session.subscribe";
      params: {
        session_id: CommonIdV1;
        from_seq: number | null;
      };
    }
  | {
      jsonrpc: "2.0";
      id: number;
      method: "session.close";
      params: {
        session_id: CommonIdV1;
      };
    }
  | {
      jsonrpc: "2.0";
      id: number;
      method: "session.list";
      params: {};
    };
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Texto em base64. Bytes em base64 padrão (RFC 4648) com preenchimento; vazio é permitido.
 */
export type CommonBase64V1 = string;
