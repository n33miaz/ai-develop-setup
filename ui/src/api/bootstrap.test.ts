import { beforeEach, describe, expect, it, vi } from 'vitest'
import { selectCoreClient } from './bootstrap'
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
