import type { CoreClient } from './client'
import { desktopSocketFactory, type DesktopBridge } from './desktop'
import { createFakeCoreClient } from './fake'
import { createCoreClient, createWsCoreClient, type WsClientOptions } from './ws'

const STORAGE_KEY = 'ads.local-api'

type ReplaySettings = Pick<WsClientOptions, 'replayEvents' | 'replayBytes'>

/** Inteiro de 1 em diante da configuração, ou `undefined` para o padrão do cliente. */
function positive(value: unknown): number | undefined {
  const parsed = Number(value)
  return value !== undefined && value !== '' && Number.isSafeInteger(parsed) && parsed >= 1
    ? parsed
    : undefined
}

/** Limites de replay da configuração da interface; ausentes ou abaixo de 1 ficam no padrão. */
export function replaySettings(events: unknown, bytes: unknown): ReplaySettings {
  const replayEvents = positive(events)
  const replayBytes = positive(bytes)
  return {
    ...(replayEvents === undefined ? {} : { replayEvents }),
    ...(replayBytes === undefined ? {} : { replayBytes }),
  }
}

export function selectCoreClient(
  browser: Pick<Window, 'location' | 'history' | 'sessionStorage'>,
  createReal: (options: WsClientOptions) => CoreClient = createWsCoreClient,
  replay: ReplaySettings = {},
): { client: CoreClient; live: boolean } {
  const fragment = new URLSearchParams(browser.location.hash.slice(1))
  let credentials: { url: string; token: string } | null = null
  const url = fragment.get('core')
  const token = fragment.get('token')
  if (fragment.has('core') || fragment.has('token')) {
    browser.history.replaceState(null, '', browser.location.pathname + browser.location.search)
  }
  if (url && token) {
    credentials = { url, token }
    try {
      browser.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(credentials))
    } catch {
      /* A conexão também funciona sem armazenamento. */
    }
  } else {
    try {
      const stored: unknown = JSON.parse(browser.sessionStorage.getItem(STORAGE_KEY) ?? 'null')
      if (
        stored &&
        typeof stored === 'object' &&
        'url' in stored &&
        'token' in stored &&
        typeof stored.url === 'string' &&
        typeof stored.token === 'string'
      ) {
        credentials = { url: stored.url, token: stored.token }
      }
    } catch {
      /* A aba sem credenciais usa os exemplos locais. */
    }
  }
  if (credentials) return { client: createReal({ ...credentials, ...replay }), live: true }
  return { client: createFakeCoreClient(), live: false }
}

export function desktopBridge(browser: Window): DesktopBridge | undefined {
  return (browser as Window & { __TAURI__?: DesktopBridge }).__TAURI__
}

export function selectDesktopClient(
  { core }: DesktopBridge,
  replay: ReplaySettings,
  retryInitial?: unknown,
  retryMax?: unknown,
  retryStable?: unknown,
  retryAttempts?: unknown,
): { client: CoreClient; live: true } {
  const delay = (value: unknown) => {
    const parsed = positive(value)
    return parsed !== undefined && parsed <= 2_147_483_647 ? parsed : undefined
  }
  const socketFactory = desktopSocketFactory(core, {
    initialMs: delay(retryInitial),
    maxMs: delay(retryMax),
    stableMs: delay(retryStable),
    maxAttempts: positive(retryAttempts),
  })
  return {
    client: createCoreClient(socketFactory, { ...replay, reconnectOnDrop: true }),
    live: true,
  }
}
