/* Gerado por tools/gen-ts.mjs a partir de schemas/record.update-manifest.v1.schema.json. Não edite à mão. */

/**
 * Instante em UTC. Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos.
 */
export type CommonTimestampV1 = string;
/**
 * Caminho relativo. Caminho relativo com barra normal, sem raiz, sem letra de unidade e sem segmento '..'.
 */
export type CommonRelpathV1 = string;
/**
 * Digest SHA-256. Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos.
 */
export type CommonSha256V1 = string;

/**
 * Manifesto de atualização. O que o CI publica a cada junção: qual build é, de qual commit saiu e o SHA-256 de cada arquivo. O lançador confere a assinatura do manifesto (arquivo à parte, sobre os bytes exatos deste JSON) com a chave pública embutida e depois o digest de cada arquivo, antes de trocar qualquer coisa.
 */
export interface RecordUpdateManifestV1 {
  version: 1;
  build: string;
  commit: string;
  created_at: CommonTimestampV1;
  /**
   * @minItems 1
   */
  files: [
    {
      path: CommonRelpathV1;
      sha256: CommonSha256V1;
    },
    ...{
      path: CommonRelpathV1;
      sha256: CommonSha256V1;
    }[]
  ];
}
