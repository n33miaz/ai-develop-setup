import { useRef, useState, type SubmitEvent } from 'react'
import { CommandRefusal, type queueApprovals } from '../api/queueApprovals'
import {
  HARNESS_NOT_CONFIGURED,
  QUEUE_ALREADY_CREATED,
  QUEUE_ALREADY_ENABLED,
  QUEUE_NOT_CONFIGURED,
} from '../../../contracts/generated/ts/contract.local-api.error-code.v4.constants'
import { useT, type MessageKey } from '../i18n'
import styles from './UsagePanel/UsagePanel.module.css'

type Api = ReturnType<typeof queueApprovals>
type State = Awaited<ReturnType<Api['get']>>['state'] | 'missing'
type Action = 'create' | 'start' | 'stop'

const refusals: Partial<Record<number, MessageKey>> = {
  [QUEUE_NOT_CONFIGURED]: 'queue.unconfigured',
  [HARNESS_NOT_CONFIGURED]: 'queue.executor',
  [QUEUE_ALREADY_CREATED]: 'queue.alreadyCreated',
  [QUEUE_ALREADY_ENABLED]: 'queue.alreadyEnabled',
}

export function QueuePanel({ api }: { api: Api }) {
  const t = useT()
  const [id, setId] = useState('')
  const [state, setState] = useState<State>()
  const [message, setMessage] = useState<MessageKey>()
  const [busy, setBusy] = useState(false)
  const lock = useRef(false)

  async function run(action?: Action) {
    if (lock.current) return
    lock.current = true
    setBusy(true)
    setMessage(undefined)
    setState(undefined)
    let changed = false
    try {
      if (action) {
        await api[action](id)
        changed = true
      }
      setState((await api.get(id)).state)
      if (action) setMessage('controls.saved')
    } catch (error) {
      if (!action && error instanceof CommandRefusal && error.code === QUEUE_NOT_CONFIGURED) {
        setState('missing')
      } else {
        setMessage(
          changed
            ? 'queue.savedRefreshFailed'
            : error instanceof CommandRefusal
              ? (refusals[error.code] ?? 'controls.refused')
              : 'controls.failed',
        )
      }
    } finally {
      lock.current = false
      setBusy(false)
    }
  }

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault()
    void run()
  }

  return (
    <details className={styles.panel}>
      <summary>{t('queue.heading')}</summary>
      <div className={styles.content}>
        <form onSubmit={submit} className={styles.picker}>
          <label>
            {t('queue.id')}{' '}
            <input
              required
              pattern={'[a-zA-Z0-9._:\\-]{1,64}'}
              maxLength={64}
              value={id}
              disabled={busy}
              onChange={(event) => {
                setId(event.target.value)
                setState(undefined)
                setMessage(undefined)
              }}
            />
          </label>
          <button disabled={busy}>{t('controls.refresh')}</button>
        </form>
        {busy && <p role="status">{t('controls.loading')}</p>}
        {message && <p role={message === 'controls.saved' ? 'status' : 'alert'}>{t(message)}</p>}
        {state && <p>{t(`queue.${state}`)}</p>}
        {state && state !== 'missing' && <p>{t('queue.limitsUnavailable')}</p>}
        {state === 'missing' && (
          <button onClick={() => void run('create')}>{t('queue.create')}</button>
        )}
        {state === 'stopped' && (
          <button onClick={() => void run('start')}>{t('queue.start')}</button>
        )}
        {state === 'enabled' && <button onClick={() => void run('stop')}>{t('queue.stop')}</button>}
      </div>
    </details>
  )
}
