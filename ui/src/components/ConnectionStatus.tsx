import type { ConnectionState } from '../api'
import { useT, type MessageKey } from '../i18n'
import styles from './ConnectionStatus.module.css'

const LABEL: Record<ConnectionState, MessageKey> = {
  disconnected: 'connection.disconnected',
  connecting: 'connection.connecting',
  connected: 'connection.connected',
}

/** Estado da conexão com o núcleo. A forma do ponto muda com o estado, para não depender só de cor. */
export function ConnectionStatus({
  state,
  desktop = false,
}: {
  state: ConnectionState
  desktop?: boolean
}) {
  const t = useT()
  const relaunch = desktop && state === 'disconnected'
  const key = relaunch ? 'connection.relaunchDesktop' : LABEL[state]
  const label = `${t('connection.label')}: ${t(key)}`
  return (
    <p
      className={[styles.status, styles[state]].join(' ')}
      role="status"
      aria-label={label}
      data-state={state}
    >
      <span className={styles.dot} aria-hidden="true" />
      <span className={styles.text} data-relaunch={relaunch} aria-hidden="true">
        {label}
      </span>
    </p>
  )
}
