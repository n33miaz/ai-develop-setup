import { describe, expect, it, vi } from 'vitest'
import { createFakeCoreClient, decodeBase64 } from './index'
import { isLocalCommand } from './wire'
import { sessionInput } from './session'

describe('session input', () => {
  it('sends unicode and control bytes in bounded ordered commands', async () => {
    const client = createFakeCoreClient()
    await client.connect()
    const send = vi.spyOn(client, 'send')
    const error = vi.fn()
    const input = sessionInput(client, 'session-a', error)
    const bytes = new TextEncoder().encode('á😀\r\x03'.repeat(20000))
    input(bytes)
    input(new Uint8Array([27, 91, 65]))
    await vi.waitFor(() => {
      expect(send).toHaveBeenCalledTimes(Math.ceil(bytes.length / 65536) + 1)
    })
    const chunks = send.mock.calls.map(([command]) => {
      expect(isLocalCommand(command)).toBe(true)
      const chunk = decodeBase64(command.params.data as string)
      expect(chunk.length).toBeLessThanOrEqual(65536)
      return [...chunk]
    })
    expect(chunks.flat()).toEqual([...bytes, 27, 91, 65])
    expect(error).not.toHaveBeenCalled()
  })

  it('stops queued writes and reports a rejection', async () => {
    const client = createFakeCoreClient()
    const send = vi
      .spyOn(client, 'send')
      .mockResolvedValue({ jsonrpc: '2.0', id: 'id', error: { code: -1, message: 'rejected' } })
    const error = vi.fn()
    sessionInput(client, 's-a', error)(new Uint8Array(70000))
    await vi.waitFor(() => {
      expect(error).toHaveBeenCalledOnce()
    })
    expect(send).toHaveBeenCalledOnce()
  })
})
