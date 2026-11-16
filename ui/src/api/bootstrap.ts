import type { CoreClient } from './client'
import { createFakeCoreClient } from './fake'
import { createWsCoreClient, type WsClientOptions } from './ws'

const STORAGE_KEY = 'ads.local-api'

export function selectCoreClient(
  browser: Pick<Window, 'location' | 'history' | 'sessionStorage'>,
  createReal: (options: WsClientOptions) => CoreClient = createWsCoreClient,
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
  if (credentials) return { client: createReal(credentials), live: true }
  return { client: createFakeCoreClient(), live: false }
}
