import { describe, expect, it } from 'vitest'
import { createFakeCoreClient, encodeBase64 } from '../../api'
import { eventByteSource } from './byteSource'

describe('event byte source', () => {
  it('decodes output bytes from its stream and stops on unsubscribe', async () => {
    const client = createFakeCoreClient()
    await client.connect()
    const bytes = new TextEncoder().encode('olá\r\n')
    const output = {
      type: 'urn:ads:event:session.output:1',
      stream: 'session/session-a',
      data: {
        session_id: 'session-a',
        payload: encodeBase64(bytes),
        length: bytes.length,
        offset: 0,
      },
    }
    client.emit(output)
    const seen: Uint8Array[] = []
    const stop = eventByteSource(client, 'session/session-a').subscribe((value) => seen.push(value))
    client.emit({
      type: 'urn:ads:event:session.resized:1',
      stream: output.stream,
      data: { session_id: 'session-a', cols: 80, rows: 24 },
    })
    client.emit({
      ...output,
      stream: 'session/session-b',
      data: { ...output.data, session_id: 'session-b' },
    })
    client.emit(output)
    stop()
    client.emit(output)
    expect(seen.map((value) => [...value])).toEqual([[...bytes], [...bytes]])
  })
  it('does not subscribe while disconnected', () => {
    expect(() =>
      eventByteSource(createFakeCoreClient(), 'session/session-a').subscribe(() => undefined),
    ).not.toThrow()
  })
})
