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

  it('drops the rest of a failed write and reports it once', async () => {
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

  it('keeps sending later writes in order after a rejected write', async () => {
    const client = createFakeCoreClient()
    const send = vi
      .spyOn(client, 'send')
      .mockResolvedValueOnce({ jsonrpc: '2.0', id: 'id', error: { code: -1, message: 'rejected' } })
      .mockResolvedValue({ jsonrpc: '2.0', id: 'id', result: {} })
    const error = vi.fn()
    const input = sessionInput(client, 's-a', error)
    input(new Uint8Array([1]))
    input(new Uint8Array([2]))
    input(new Uint8Array([3]))
    await vi.waitFor(() => {
      expect(send).toHaveBeenCalledTimes(3)
    })
    const sent = send.mock.calls.map(([command]) => [
      ...decodeBase64(command.params.data as string),
    ])
    expect(sent).toEqual([[1], [2], [3]])
    expect(error).toHaveBeenCalledOnce()
  })

  it('waits for each write before the next, even when one throws', async () => {
    const client = createFakeCoreClient()
    let fail: (reason: Error) => void = () => undefined
    const send = vi
      .spyOn(client, 'send')
      .mockReturnValueOnce(
        new Promise((_, reject) => {
          fail = reject
        }),
      )
      .mockResolvedValue({ jsonrpc: '2.0', id: 'id', result: {} })
    const error = vi.fn()
    const input = sessionInput(client, 's-a', error)
    input(new Uint8Array([1]))
    input(new Uint8Array([2]))
    input(new Uint8Array([3]))
    await new Promise((resolve) => setTimeout(resolve, 10))
    expect(send).toHaveBeenCalledOnce()
    fail(new Error('socket closed'))
    await vi.waitFor(() => {
      expect(send).toHaveBeenCalledTimes(3)
    })
    const sent = send.mock.calls.map(([command]) => [
      ...decodeBase64(command.params.data as string),
    ])
    expect(sent).toEqual([[1], [2], [3]])
    expect(error).toHaveBeenCalledOnce()
  })

  it('does not report again for empty input after a failure', async () => {
    const client = createFakeCoreClient()
    vi.spyOn(client, 'send').mockResolvedValue({
      jsonrpc: '2.0',
      id: 'id',
      error: { code: -1, message: 'rejected' },
    })
    const error = vi.fn()
    const input = sessionInput(client, 's-a', error)
    input(new Uint8Array([1]))
    await vi.waitFor(() => {
      expect(error).toHaveBeenCalledOnce()
    })
    input(new Uint8Array(0))
    await new Promise((resolve) => setTimeout(resolve, 10))
    expect(error).toHaveBeenCalledOnce()
  })
})
