import { describe, expect, it, vi } from 'vitest'
import { registerPwa } from './register'
import { staticResponse } from './worker.js'

const base = 'https://app.example.test/ui/'

it('defines a standalone installation within the build directory', async () => {
  const { default: source } = await import('../../public/manifest.webmanifest?raw')
  const manifest: unknown = JSON.parse(source)
  expect(manifest).toMatchObject({
    name: 'ai-develop-setup',
    short_name: 'ai-develop-setup',
    display: 'standalone',
    start_url: './',
    scope: './',
    theme_color: expect.stringMatching(/^#[0-9a-f]{6}$/i) as string,
    background_color: expect.stringMatching(/^#[0-9a-f]{6}$/i) as string,
    icons: [
      { src: './icon-192.png', sizes: '192x192', type: 'image/png' },
      { src: './icon-512.png', sizes: '512x512', type: 'image/png' },
    ],
  })
})

describe('static worker requests', () => {
  const asset = base + 'assets/index-hash.js'
  const allowed = new Set([asset])

  it('reads an allowlisted asset from the build cache', async () => {
    const response = new Response('static content')
    const match = vi.fn().mockResolvedValue(response)
    await expect(staticResponse(new Request(asset), allowed, { match }, 'build')).resolves.toBe(
      response,
    )
    expect(match).toHaveBeenCalledExactlyOnceWith(asset, { cacheName: 'build' })
  })

  it.each([
    { url: base + 'api/session' },
    { url: base + 'assets/other.js' },
    { url: 'https://other.example.test/ui/assets/index-hash.js' },
    { url: 'ws://127.0.0.1:8000/' },
    { url: 'wss://app.example.test/socket' },
    { url: asset + '?token=example' },
    { url: asset + '#token=example' },
    { url: base + '#core=example&token=example', mode: 'navigate' },
    { url: asset, mode: 'navigate' },
    { url: asset, method: 'POST' },
  ] as const)('ignores $url outside the static request policy', (input) => {
    const match = vi.fn()
    const request = { method: 'GET', mode: 'cors', ...input } as const
    expect(staticResponse(request, allowed, { match }, 'build')).toBeUndefined()
    expect(match).not.toHaveBeenCalled()
  })
})

describe('service worker registration', () => {
  function browser() {
    const worker = new EventTarget()
    const postMessage = vi.fn()
    const getRegistration = vi.fn().mockResolvedValue({ waiting: { postMessage } })
    const register = vi.fn().mockResolvedValue(worker)
    const serviceWorker = {
      getRegistration,
      register,
      controller: null,
    } as unknown as ServiceWorkerContainer
    return { getRegistration, register, serviceWorker, postMessage }
  }

  it.each([
    [false, true],
    [true, false],
    [false, false],
  ])('skips registration with production=%s and secure=%s', async (production, secure) => {
    const { getRegistration, register, serviceWorker } = browser()
    await registerPwa(production, secure, serviceWorker, base, vi.fn())
    expect(getRegistration).not.toHaveBeenCalled()
    expect(register).not.toHaveBeenCalled()
  })

  it('registers in the build directory and activates a previously waiting update', async () => {
    const { register, serviceWorker, postMessage } = browser()
    await registerPwa(true, true, serviceWorker, base, vi.fn())
    expect(register).toHaveBeenCalledExactlyOnceWith(new URL('service-worker.js', base), {
      type: 'module',
    })
    expect(postMessage).toHaveBeenCalledExactlyOnceWith('activate-update')
  })

  it('announces a newly installed update without activating it on the same page', async () => {
    const installing = Object.assign(new EventTarget(), { state: 'installing' })
    const registration = Object.assign(new EventTarget(), { installing: null })
    const onUpdate = vi.fn()
    const serviceWorker = {
      getRegistration: vi.fn().mockResolvedValue(undefined),
      register: vi.fn().mockResolvedValue(registration),
      controller: {},
    } as unknown as ServiceWorkerContainer
    await registerPwa(true, true, serviceWorker, base, onUpdate)
    Object.assign(registration, { installing })
    registration.dispatchEvent(new Event('updatefound'))
    expect(onUpdate).not.toHaveBeenCalled()
    installing.state = 'installed'
    installing.dispatchEvent(new Event('statechange'))
    expect(onUpdate).toHaveBeenCalledOnce()
  })
})
