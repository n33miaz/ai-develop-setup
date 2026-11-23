/* Gerado por tools/gen-ts.mjs a partir de schemas/command.approval.close.v1.schema.json. Não edite à mão. */

/**
 * Fechar um pedido de aprovação ilegível. Recuperação: o dono fecha, pelo id da ação, os pedidos de aprovação pendentes de uma ação cujo histórico o núcleo não consegue ler (o mesmo id que o log do daemon registra quando deixa o pedido fora de approval.list). A decisão é negar (deny) ou expirar (expire); nunca aprova e nunca emite licença. Um pedido com histórico legível é recusado com APPROVAL_READABLE e continua sendo respondido por approval.answer. O dono e o aparelho vêm do núcleo, nunca do pedido. A resposta é command.response:2.
 */
export type CommandApprovalCloseV1 = CommandRequestV1 & {
  method: "approval.close";
  params: {
    action_id: CommonIdV1;
    decision: "deny" | "expire";
    _meta: unknown;
  };
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
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Envelope de comando. Comando JSON-RPC 2.0 enviado ao núcleo. Comando é pedido, não fato; a chave de idempotência evita executar duas vezes, e a origem remota exige assinatura do aparelho, nonce, contador e validade.
 */
export interface CommandRequestV1 {
  jsonrpc: "2.0";
  id: CommonUlidV1;
  method: string;
  params: {
    _meta: {
      idempotency_key: CommonUlidV1;
      origin: "local" | "remote";
      remote?: {
        device_id: CommonIdV1;
        signature: string;
        nonce: string;
        counter: number;
        expires_at: CommonTimestampV1;
      };
    };
    [k: string]: unknown;
  };
}
