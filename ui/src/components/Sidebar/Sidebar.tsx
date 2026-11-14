import { useT, type MessageKey } from '../../i18n'
import type { SampleSession, SampleTask } from '../../sample/data'
import styles from './Sidebar.module.css'

interface SidebarProps {
  id: string
  open: boolean
  tasks: readonly SampleTask[]
  sessions: readonly SampleSession[]
  selectedSessionId: string
  onSelectSession: (id: string) => void
}

const TASK_STATE: Record<SampleTask['state'], MessageKey> = {
  pending: 'task.state.pending',
  running: 'task.state.running',
  done: 'task.state.done',
}

const SESSION_STATE: Record<SampleSession['state'], MessageKey> = {
  active: 'session.state.active',
  idle: 'session.state.idle',
}

/** Caixa de tarefas e lista de sessões. Só mostra o que recebe e avisa qual sessão foi escolhida. */
export function Sidebar({
  id,
  open,
  tasks,
  sessions,
  selectedSessionId,
  onSelectSession,
}: SidebarProps) {
  const t = useT()
  return (
    <nav id={id} className={styles.sidebar} data-open={open} aria-label={t('nav.label')}>
      <section className={styles.section} aria-labelledby={`${id}-tasks`}>
        <h2 id={`${id}-tasks`}>{t('tasks.heading')}</h2>
        {tasks.length === 0 ? (
          <p className={styles.empty}>{t('tasks.empty')}</p>
        ) : (
          <ul className={styles.list}>
            {tasks.map((task) => (
              <li key={task.id} className={styles.item}>
                <span>{t(task.titleKey)}</span>
                <span className={styles.state}>{t(TASK_STATE[task.state])}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
      <section className={styles.section} aria-labelledby={`${id}-sessions`}>
        <h2 id={`${id}-sessions`}>{t('sessions.heading')}</h2>
        {sessions.length === 0 ? (
          <p className={styles.empty}>{t('sessions.empty')}</p>
        ) : (
          <ul className={styles.list}>
            {sessions.map((session) => (
              <li key={session.id}>
                <button
                  type="button"
                  className={styles.item}
                  aria-current={session.id === selectedSessionId}
                  onClick={() => {
                    onSelectSession(session.id)
                  }}
                >
                  <span>{t(session.nameKey)}</span>
                  <span className={styles.state}>{t(SESSION_STATE[session.state])}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </nav>
  )
}
