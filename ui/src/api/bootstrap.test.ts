import { beforeEach, describe, expect, it, vi } from 'vitest'
import { replaySettings, selectCoreClient, selectDesktopClient } from './bootstrap'
import { newCommand } from './command'
import { createFakeCoreClient } from './fake'

beforeEach(() => {
  window.sessionStorage.clear()
  window.history.replaceState(null, '', '/')
})

describe('client selection', () => {
  it('consumes the fragment, stores tab credentials and selects the real client', () => {
    const token = 'a'.repeat(43)
    window.history.replaceState(
      null,
      '',
      `/?view=terminal#core=ws://127.0.0.1:12345&token=${token}`,
    )
    const client = createFakeCoreClient()
    const real = vi.fn(() => client)
    expect(selectCoreClient(window, real)).toEqual({ client, live: true })
    expect(real).toHaveBeenCalledWith({ url: 'ws://127.0.0.1:12345', token })
    // The replay limits from the interface configuration reach the client.
    selectCoreClient(window, real, { replayEvents: 0, replayBytes: 1024 })
    expect(real).toHaveBeenLastCalledWith({
      url: 'ws://127.0.0.1:12345',
      token,
      replayEvents: 0,
      replayBytes: 1024,
    })
    expect(window.location.hash).toBe('')
    expect(window.location.search).toBe('?view=terminal')
    expect(JSON.parse(window.sessionStorage.getItem('ads.local-api') ?? 'null')).toEqual({
      url: 'ws://127.0.0.1:12345',
      token,
    })
  })
  it('keeps examples as the default without credentials', () => {
    const real = vi.fn(() => createFakeCoreClient())
    const result = selectCoreClient(window, real)
    expect(result.live).toBe(false)
    expect(result.client.getState()).toBe('disconnected')
    expect(real).not.toHaveBeenCalled()
  })
  it('restores credentials only from the current tab', () => {
    window.sessionStorage.setItem(
      'ads.local-api',
      JSON.stringify({ url: 'ws://localhost:12345', token: 'a'.repeat(43) }),
    )
    expect(selectCoreClient(window).live).toBe(true)
  })
  it('works when access to session storage throws', () => {
    const browser = {
      location: window.location,
      history: window.history,
      get sessionStorage(): Storage {
        throw new Error('storage unavailable')
      },
    }
    expect(selectCoreClient(browser).live).toBe(false)
    window.history.replaceState(null, '', `/#core=ws://localhost:12345&token=${'a'.repeat(43)}`)
    expect(selectCoreClient(browser).live).toBe(true)
    expect(window.location.hash).toBe('')
  })
  it('ignores malformed stored data', () => {
    window.sessionStorage.setItem('ads.local-api', '{')
    expect(selectCoreClient(window).live).toBe(false)
  })
})

describe('desktop client selection', () => {
  type Frame = { kind: 'message'; text: string } | { kind: 'closed' }
  class Channel {
    onmessage: (frame: Frame) => void = () => undefined
  }

  it('retries transient failures with increasing delays up to the configured cap', async () => {
    vi.useFakeTimers()
    const invoke = vi.fn(() => Promise.resolve())
    for (let failure = 0; failure < 3; failure++) {
      invoke.mockRejectedValueOnce('desktop connection retry')
    }
    const { client } = selectDesktopClient({ core: { invoke, Channel } }, {}, '25', '50')
    try {
      const connecting = client.connect()
      await vi.advanceTimersByTimeAsync(24)
      expect(invoke).toHaveBeenCalledTimes(1)
      expect(client.getState()).toBe('connecting')
      await vi.advanceTimersByTimeAsync(1)
      expect(invoke).toHaveBeenCalledTimes(2)
      await vi.advanceTimersByTimeAsync(49)
      expect(invoke).toHaveBeenCalledTimes(2)
      await vi.advanceTimersByTimeAsync(1)
      expect(invoke).toHaveBeenCalledTimes(3)
      await vi.advanceTimersByTimeAsync(50)
      expect(invoke).toHaveBeenCalledTimes(4)
      await expect(connecting).resolves.toBeUndefined()
      expect(client.getState()).toBe('connected')
    } finally {
      client.disconnect()
      vi.useRealTimers()
    }
  })

  it.each([
    ['desktop identity proof missing', 'proof_missing'],
    ['desktop relaunch required', 'relaunch_required'],
  ])('reports %s as %s', async (reason, state) => {
    const invoke = vi.fn(() => Promise.resolve()).mockRejectedValueOnce(reason)
    const { client } = selectDesktopClient({ core: { invoke, Channel } }, {})
    expect(client.getState()).toBe('disconnected')
    await expect(client.connect()).rejects.toMatchObject({ code: 'not_connected' })
    expect(client.getState()).toBe(state)
  })

  it.each([{ kind: 'closed' }, { kind: 'message', text: '{' }] satisfies Frame[])(
    'backs off across short-lived connections that end with $kind',
    async (frame) => {
      vi.useFakeTimers()
      let channel: Channel | undefined
      const connect = vi.fn()
      const invoke = (command: string, args: Record<string, unknown>) => {
        if (command === 'desktop_connect') {
          connect()
          channel = args.frames as Channel
        }
        return Promise.resolve()
      }
      const { client } = selectDesktopClient({ core: { invoke, Channel } }, {}, 250, 1000, 2000)
      try {
        await client.connect()
        for (const [index, delay] of [250, 500, 1000, 1000].entries()) {
          channel?.onmessage(frame)
          await vi.advanceTimersByTimeAsync(delay - 1)
          expect(connect).toHaveBeenCalledTimes(index + 1)
          await vi.advanceTimersByTimeAsync(1)
          expect(connect).toHaveBeenCalledTimes(index + 2)
          expect(client.getState()).toBe('connected')
        }
        await vi.advanceTimersByTimeAsync(2000)
        channel?.onmessage(frame)
        await vi.advanceTimersByTimeAsync(250)
        expect(connect).toHaveBeenCalledTimes(6)
      } finally {
        client.disconnect()
        vi.useRealTimers()
      }
    },
  )

  it.each(['desktop connection unavailable', 'desktop connection retry'])(
    'retries %s after a drop until the attempt limit',
    async (reason) => {
      vi.useFakeTimers()
      let channel: Channel | undefined
      const connect = vi
        .fn<() => Promise<void>>()
        .mockResolvedValueOnce(undefined)
        .mockRejectedValue(reason)
      const invoke = (command: string, args: Record<string, unknown>) => {
        if (command !== 'desktop_connect') return Promise.resolve()
        channel = args.frames as Channel
        return connect()
      }
      const { client } = selectDesktopClient({ core: { invoke, Channel } }, {}, 25, 50, 2000, 2)
      try {
        await client.connect()
        channel?.onmessage({ kind: 'closed' })
        await vi.advanceTimersByTimeAsync(74)
        expect(connect).toHaveBeenCalledTimes(2)
        expect(client.getState()).toBe('connecting')
        await vi.advanceTimersByTimeAsync(1)
        expect(client.getState()).toBe('retry_exhausted')
        await vi.advanceTimersByTimeAsync(1000)
        expect(connect).toHaveBeenCalledTimes(3)
      } finally {
        client.disconnect()
        vi.useRealTimers()
      }
    },
  )

  it('stops reconnecting when the replacement daemon fails the identity check', async () => {
    vi.useFakeTimers()
    let channel: Channel | undefined
    const connect = vi
      .fn<() => Promise<void>>()
      .mockResolvedValueOnce(undefined)
      .mockRejectedValue('desktop relaunch required')
    const invoke = vi.fn((command: string, args: Record<string, unknown>) => {
      if (command !== 'desktop_connect') return Promise.resolve()
      channel ??= args.frames as Channel
      return connect()
    })
    const { client } = selectDesktopClient({ core: { invoke, Channel } }, {}, 25, 50)
    try {
      await client.connect()
      channel?.onmessage({ kind: 'closed' })
      await expect(
        client.send(newCommand('session.write', { session_id: 'session-a', data: 'YQ==' })),
      ).rejects.toMatchObject({ code: 'not_connected' })
      await vi.advanceTimersByTimeAsync(500)
      expect(client.getState()).toBe('relaunch_required')
      expect(client.getGeneration()).toBe(1)
      expect(invoke.mock.calls.map(([command]) => command)).toEqual([
        'desktop_connect',
        'desktop_disconnect',
        'desktop_connect',
        'desktop_disconnect',
      ])
    } finally {
      client.disconnect()
      vi.useRealTimers()
    }
  })

  it('uses scoped IPC for commands and reconnects without exchanging credentials', async () => {
    vi.useFakeTimers()
    let frames: Channel
    let available = true
    const retry = vi.fn<() => Promise<void>>().mockRejectedValue('desktop connection retry')
    const invoke = vi.fn((command: string, args: Record<string, unknown>) => {
      if (command === 'desktop_connect') {
        if (!available) return retry()
        frames = args.frames as Channel
      }
      if (command === 'desktop_send') {
        const request = JSON.parse(String(args.text)) as { id: string; method: string }
        expect(request.method).toBe('harness.list')
        frames.onmessage({
          kind: 'message',
          text: JSON.stringify({
            jsonrpc: '2.0',
            id: request.id,
            result: { harnesses: [], workdir_aliases: [] },
          }),
        })
      }
      return Promise.resolve()
    })
    window.history.replaceState(null, '', '/#untouched')
    const { client } = selectDesktopClient({ core: { invoke, Channel } }, {}, 25, 50)
    try {
      await client.connect()
      const first = invoke.mock.calls[0]?.[1]
      expect(Object.keys(first ?? {}).sort()).toEqual(['frames', 'id'])
      await expect(client.listHarnesses()).resolves.toEqual({ harnesses: [], workdir_aliases: [] })

      const firstChannel = first?.frames as Channel
      available = false
      firstChannel.onmessage({ kind: 'closed' })
      await expect(
        client.send(newCommand('session.write', { session_id: 'session-a', data: 'YQ==' })),
      ).rejects.toMatchObject({ code: 'not_connected' })
      expect(client.getState()).toBe('connecting')
      await vi.advanceTimersByTimeAsync(24)
      expect(invoke.mock.calls.filter(([command]) => command === 'desktop_connect')).toHaveLength(1)
      await vi.advanceTimersByTimeAsync(1)
      expect(invoke.mock.calls.filter(([command]) => command === 'desktop_connect')).toHaveLength(2)
      expect(client.getGeneration()).toBe(1)
      available = true
      await vi.advanceTimersByTimeAsync(49)
      expect(invoke.mock.calls.filter(([command]) => command === 'desktop_connect')).toHaveLength(2)
      await vi.advanceTimersByTimeAsync(1)
      expect(invoke.mock.calls.filter(([command]) => command === 'desktop_connect')).toHaveLength(3)
      expect(client.getGeneration()).toBe(2)
      expect(client.getState()).toBe('connected')
      expect(invoke.mock.calls.filter(([command]) => command === 'desktop_send')).toHaveLength(1)
      await expect(client.listHarnesses()).resolves.toEqual({ harnesses: [], workdir_aliases: [] })
      expect(window.location.hash).toBe('#untouched')
      expect(window.sessionStorage.length).toBe(0)
      expect(window.localStorage.getItem('ads.local-api')).toBeNull()
      const latest = invoke.mock.calls.filter(([command]) => command === 'desktop_connect').at(-1)
      expect(latest?.[1].id).not.toBe(first?.id)
      const latestChannel = latest?.[1].frames as Channel
      latestChannel.onmessage({ kind: 'closed' })
      client.disconnect()
      await vi.advanceTimersByTimeAsync(500)
      expect(invoke.mock.calls.filter(([command]) => command === 'desktop_connect')).toHaveLength(3)
    } finally {
      client.disconnect()
      vi.useRealTimers()
    }
  })
})

describe('replay settings', () => {
  it('keeps limits from 1 up and leaves the rest to the client default', () => {
    expect(replaySettings('1', '2048')).toEqual({ replayEvents: 1, replayBytes: 2048 })
    // With 0 nothing would be kept: a remounted terminal would stay empty.
    expect(replaySettings('0', '0')).toEqual({})
    expect(replaySettings('-3', '1.5')).toEqual({})
    expect(replaySettings(undefined, '')).toEqual({})
    expect(replaySettings('abc', 'NaN')).toEqual({})
  })
})
