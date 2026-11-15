/** De onde o terminal recebe bytes. Quem assina decide o que fazer com cada bloco. */
export interface ByteSource {
  subscribe(onBytes: (bytes: Uint8Array) => void): () => void
}

/** Fonte de bytes fixos, entregues assim que alguém assina. Serve aos dados de exemplo. */
export function staticByteSource(content: string | Uint8Array): ByteSource {
  const bytes = typeof content === 'string' ? new TextEncoder().encode(content) : content
  return {
    subscribe(onBytes) {
      onBytes(bytes)
      return () => undefined
    },
  }
}
