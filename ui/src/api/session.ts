import { encodeBase64, newCommand, type CoreClient } from './index'

export function sessionInput(client: CoreClient, sessionId: string, onError: () => void) {
  let pending = Promise.resolve()

  return (bytes: Uint8Array) => {
    for (let offset = 0; offset < bytes.length; offset += 65536) {
      const data = encodeBase64(bytes.slice(offset, offset + 65536))

      pending = pending.then(async () => {
        const response = await client.send(
          newCommand('session.write', { session_id: sessionId, data }),
        )

        if (response.error) throw new Error('session write rejected')
      })
    }

    void pending.catch(onError)
  }
}
