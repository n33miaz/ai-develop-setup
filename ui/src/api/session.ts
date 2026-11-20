import { encodeBase64, newCommand, type CoreClient } from './index'

export function sessionInput(
  client: CoreClient,
  sessionId: string,
  onError: () => void,
  onSuccess: () => void = () => undefined,
) {
  let pending = Promise.resolve()

  return (bytes: Uint8Array) => {
    if (bytes.length === 0) return

    const chunks: string[] = []
    for (let offset = 0; offset < bytes.length; offset += 65536) {
      chunks.push(encodeBase64(bytes.slice(offset, offset + 65536)))
    }

    // Serial and in order. A failed chunk drops the rest of its own write (a
    // half-rejected paste must not leak its tail), is reported once, and the
    // writes queued after it are still sent.
    pending = pending.then(async () => {
      let sent = true
      try {
        for (const data of chunks) {
          const response = await client.send(
            newCommand('session.write', { session_id: sessionId, data }),
          )

          if (response.error) throw new Error('session write rejected')
        }
      } catch {
        sent = false
      }

      try {
        if (sent) onSuccess()
        else onError()
      } catch {
        // A failing reporter must not stall the queue.
      }
    })
  }
}
