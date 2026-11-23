/* Gerado por tools/gen-ts.mjs a partir de schemas/command.approval.list.v1.schema.json. Não edite à mão. */

/**
 * Listar aprovações pendentes. Pede ao núcleo as ações do portão que aguardam a resposta do dono, cada uma com o plano exato do ensaio. Só leitura: não muda nada no núcleo e não entra na cache de idempotência. A resposta aceita é command.approval.list.response:1; a recusa volta como command.response:2 com error.
 */
export type CommandApprovalListV1 = CommandRequestV1 & {
  method: "approval.list";
  params: {
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
