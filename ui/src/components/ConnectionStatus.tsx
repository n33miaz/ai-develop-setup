import type { ConnectionState } from '../api'
import { useT, type MessageKey } from '../i18n'
import styles from './ConnectionStatus.module.css'

const LABEL: Record<ConnectionState, MessageKey> = {
  disconnected: 'connection.disconnected',
  connecting: 'connection.connecting',
  connected: 'connection.connected',
  link_unavailable: 'connection.linkUnavailable',
  proof_missing: 'connection.proofMissing',
  relaunch_required: 'connection.relaunchDesktop',
  retry_exhausted: 'connection.retryExhausted',
}

/** Estado da conexão com o núcleo. A forma do ponto muda com o estado, para não depender só de cor. */
export function ConnectionStatus({ state }: { state: ConnectionState }) {
  const t = useT()
  const actionable =
    state === 'link_unavailable' ||
    state === 'proof_missing' ||
    state === 'relaunch_required' ||
    state === 'retry_exhausted'
  const label = `${t('connection.label')}: ${t(LABEL[state])}`
  return (
    <p
      className={[styles.status, styles[actionable ? 'disconnected' : state]].join(' ')}
      role="status"
      aria-label={label}
      data-state={state}
    >
      <span className={styles.dot} aria-hidden="true" />
      <span className={styles.text} data-actionable={actionable} aria-hidden="true">
        {label}
      </span>
    </p>
  )
}
