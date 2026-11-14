import { useCallback, useSyncExternalStore } from 'react'
import type { CoreClient } from './client'
import type { ConnectionState } from './types'

/** Estado da conexão do cliente, sempre o atual: a tela não guarda cópia própria. */
export function useConnectionState(client: CoreClient): ConnectionState {
  const subscribe = useCallback((onChange: () => void) => client.onStateChange(onChange), [client])
  return useSyncExternalStore(subscribe, () => client.getState())
}
