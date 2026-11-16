/* Gerado por tools/gen-ts.mjs a partir de schemas/command.session.write.v1.schema.json. Não edite à mão. */

/**
 * Escrever na sessão. Bytes digitados ou colados pela interface, em base64, para a entrada do terminal da sessão. De 1 a 65536 bytes por comando (87384 caracteres em base64): cabe com folga no quadro padrão do contrato pty-host/1; texto maior vai em vários comandos, na ordem.
 */
export type CommandSessionWriteV1 = CommandRequestV1 & {
  method: "session.write";
  params: {
    _meta: unknown;
    session_id: CommonIdV1;
    /**
     * Bytes em base64 padrão (RFC 4648) com preenchimento: de 1 a 65536 bytes.
     */
    data: string;
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
