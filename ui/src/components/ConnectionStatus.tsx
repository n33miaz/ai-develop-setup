import type { ConnectionState } from '../api'
import { useT, type MessageKey } from '../i18n'
import styles from './ConnectionStatus.module.css'

const LABEL: Record<ConnectionState, MessageKey> = {
  disconnected: 'connection.disconnected',
  connecting: 'connection.connecting',
  connected: 'connection.connected',
}

/** Estado da conexão com o núcleo. A forma do ponto muda com o estado, para não depender só de cor. */
export function ConnectionStatus({ state }: { state: ConnectionState }) {
  const t = useT()
  return (
    <p className={[styles.status, styles[state]].join(' ')} role="status" data-state={state}>
      <span className={styles.dot} aria-hidden="true" />
      <span className={styles.text}>
        {t('connection.label')}: {t(LABEL[state])}
      </span>
    </p>
  )
}
