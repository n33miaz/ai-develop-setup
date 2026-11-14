/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.pty-host.notification.v1.schema.json. Não edite à mão. */

/**
 * Aviso do host de PTY. Notificação JSON-RPC 2.0 (sem id, sem resposta) do host de PTY ao núcleo: o processo de uma sessão terminou, ou uma assinatura foi encerrada. last_seq é o último pedaço numerado (0 se não houve saída); o núcleo reassina a partir de last_seq + 1 sem perder nada.
 */
export type ContractPtyHostNotificationV1 =
  | {
      jsonrpc: "2.0";
      method: "session.exited";
      params: {
        session_id: CommonIdV1;
        exit_code: number | null;
        last_seq: number;
      };
    }
  | {
      jsonrpc: "2.0";
      method: "subscription.closed";
      params: {
        session_id: CommonIdV1;
        reason: "slow_subscriber" | "session_closed";
        last_seq: number;
      };
    };
/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
