/* Gerado por tools/gen-ts.mjs a partir de schemas/relay.envelope.v1.schema.json. Não edite à mão. */

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
 * Mensagem cifrada do relé. A única coisa que o relé transporta e guarda na caixa de mensagens: texto cifrado de ponta a ponta, de um aparelho para outro, com validade. O relé nunca vê texto em claro nem credencial; depois da validade a mensagem é descartada.
 */
export interface RelayEnvelopeV1 {
  id: CommonUlidV1;
  from: CommonIdV1;
  to: CommonIdV1;
  expires_at: CommonTimestampV1;
  ciphertext: string;
}
