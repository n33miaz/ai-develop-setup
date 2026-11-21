import type { CoreClient } from './client'
import { createFakeCoreClient } from './fake'
import { createWsCoreClient, type WsClientOptions } from './ws'

const STORAGE_KEY = 'ads.local-api'

type ReplaySettings = Pick<WsClientOptions, 'replayEvents' | 'replayBytes'>

/** Inteiro de 1 em diante da configuraÃ§Ã£o, ou `undefined` para o padrÃ£o do cliente. */
function positive(value: unknown): number | undefined {
  const parsed = Number(value)
  return value !== undefined && value !== '' && Number.isSafeInteger(parsed) && parsed >= 1
    ? parsed
    : undefined
}

/** Limites de replay da configuraÃ§Ã£o da interface; ausentes ou abaixo de 1 ficam no padrÃ£o. */
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
      /* A conexÃ£o tambÃ©m funciona sem armazenamento. */
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

type DesktopBridge = { core: { invoke: <T>(command: string) => Promise<T> } }

export function desktopBridge(browser: Window): DesktopBridge | undefined {
  return (browser as Window & { __TAURI__?: DesktopBridge }).__TAURI__
}

export async function selectDesktopClient(
  bridge: DesktopBridge,
  replay: ReplaySettings,
  createReal: (options: WsClientOptions) => CoreClient = createWsCoreClient,
): Promise<{ client: CoreClient; live: true }> {
  const credentials = await bridge.core.invoke<{ url: string; token: string }>('desktop_bootstrap')
  const client = createReal({ ...credentials, ...replay })
  const unsubscribe = client.onStateChange((state) => {
    if (state === 'connected') {
      unsubscribe()
      void bridge.core.invoke('desktop_connected').catch(() => undefined)
    }
  })
  return { client, live: true }
}
