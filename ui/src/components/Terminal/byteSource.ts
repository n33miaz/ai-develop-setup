/** De onde o terminal recebe bytes. Quem assina decide o que fazer com cada bloco. */
export interface ByteSource {
  subscribe(onBytes: (bytes: Uint8Array) => void): () => void
}

/** Fonte de texto fixo, entregue assim que alguém assina. Serve aos dados de exemplo. */
export function staticByteSource(text: string): ByteSource {
  const bytes = new TextEncoder().encode(text)
  return {
    subscribe(onBytes) {
      onBytes(bytes)
      return () => undefined
    },
  }
}
