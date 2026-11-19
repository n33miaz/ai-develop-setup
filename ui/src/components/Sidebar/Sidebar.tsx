import { useT, type MessageKey } from '../../i18n'
import type { SessionActivity, SessionView, TaskState, TaskView } from '../../model/views'
import styles from './Sidebar.module.css'

interface SidebarProps {
  id: string
  open: boolean
  hidden?: boolean
  tasks: readonly TaskView[]
  sessions: readonly SessionView[]
  selectedSessionId: string | null
  onSelectSession: (id: string) => void
}

const TASK_STATE: Record<TaskState, MessageKey> = {
  pending: 'task.state.pending',
  running: 'task.state.running',
  done: 'task.state.done',
  failed: 'task.state.failed',
  cancelled: 'task.state.cancelled',
}

const SESSION_ACTIVITY: Record<SessionActivity, MessageKey> = {
  active: 'session.state.active',
  idle: 'session.state.idle',
}

/**
 * Caixa de tarefas e lista de sessões. Só mostra o que recebe e avisa qual sessão foi escolhida.
 * A forma do ponto e o texto mudam com a atividade da sessão, para não depender só de cor.
 */
export function Sidebar({
  id,
  open,
  hidden,
  tasks,
  sessions,
  selectedSessionId,
  onSelectSession,
}: SidebarProps) {
  const t = useT()
  return (
    <nav
      hidden={hidden}
      id={id}
      className={styles.sidebar}
      data-open={open}
      aria-label={t('nav.label')}
    >
      <section className={styles.section} aria-labelledby={`${id}-tasks`}>
        <h2 id={`${id}-tasks`}>{t('tasks.heading')}</h2>
        {tasks.length === 0 ? (
          <p className={styles.empty}>{t('tasks.empty')}</p>
        ) : (
          <ul className={styles.list}>
            {tasks.map((task) => (
              <li key={task.id} className={styles.item}>
                <span className={styles.name}>{task.title ?? t('task.untitled')}</span>
                <span className={styles.state} data-state={task.state}>
                  {t(TASK_STATE[task.state])}
                </span>
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
                  <span className={styles.name}>{session.displayName ?? t('session.unnamed')}</span>
                  <span className={styles.state} data-activity={session.activity}>
                    <span className={styles.dot} aria-hidden="true" />
                    {t(SESSION_ACTIVITY[session.activity])}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </nav>
  )
}
