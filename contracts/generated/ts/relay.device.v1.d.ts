/* Gerado por tools/gen-ts.mjs a partir de schemas/relay.device.v1.schema.json. Não edite à mão. */

/**
 * Identificador opaco. Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba.
 */
export type CommonIdV1 = string;
/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;

/**
 * Aparelho inscrito no relé. O que o relé guarda de um aparelho: o identificador opaco e a chave pública (32 bytes em base64). Chave privada, passkey e credencial de IA nunca chegam ao relé; a passkey é conferida pelo núcleo, dentro do canal cifrado.
 */
export interface RelayDeviceV1 {
  device_id: CommonIdV1;
  public_key: string;
  registered_at: CommonTimestampV1;
}
