import { decodeBase64, type CoreClient, type EventSessionOutputV1 } from '../../api'

/** De onde o terminal recebe bytes. Quem assina decide o que fazer com cada bloco. */
export interface ByteSource {
  subscribe(onBytes: (bytes: Uint8Array, replay?: boolean) => void): () => void
}

export function eventByteSource(client: CoreClient, stream: string): ByteSource {
  return {
    subscribe(onBytes) {
      if (client.getState() !== 'connected') return () => undefined

      const subscribedAt = Date.now()

      return client.subscribe(stream, 0, (event) => {
        if (event.type !== 'urn:ads:event:session.output:1') return
        const output = event as EventSessionOutputV1
        if (output.stream !== `session/${output.data.session_id}`) return

        onBytes(decodeBase64(output.data.payload), Date.parse(event.ts) <= subscribedAt)
      })
    },
  }
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
