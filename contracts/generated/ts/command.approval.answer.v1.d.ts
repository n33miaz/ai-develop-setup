/* Gerado por tools/gen-ts.mjs a partir de schemas/command.approval.answer.v1.schema.json. Não edite à mão. */

/**
 * Responder uma aprovação. Aprova ou nega uma ação que aguarda o dono, pelo id da ação e da aprovação e pelo hash do plano que o dono viu. O dono e o aparelho vêm do núcleo, nunca do pedido. A resposta é command.response:2.
 */
export type CommandApprovalAnswerV1 = CommandRequestV1 & {
  method: "approval.answer";
  params: {
    action_id: CommonIdV1;
    approval_id: CommonIdV1;
    plan_hash: CommonSha256V1;
    decision: "approve" | "deny";
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
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;

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
