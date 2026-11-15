import type { EventDraft, EventSessionActivityChangedV1, SessionEvent, TaskEvent } from '../api'

// O que a barra lateral mostra de cada tarefa e de cada sessão, reconstruído só dos eventos.

/** Estado local da tela: os contratos trazem os eventos da tarefa, não um estado pronto. */
export type TaskState = 'pending' | 'running' | 'done'

export type SessionActivity = EventSessionActivityChangedV1['data']['activity']

export interface TaskView {
  id: string
  /** O `title` de `task.created`; ausente quando o evento não o traz. */
  title?: string
  state: TaskState
}

export interface SessionView {
  id: string
  /** O `display_name` de `session.created`; ausente quando o evento não o traz. */
  displayName?: string
  activity: SessionActivity
}

const TASK_STATE_AFTER: Partial<Record<TaskEvent['type'], TaskState>> = {
  'urn:ads:event:task.started:1': 'running',
  'urn:ads:event:task.completed:1': 'done',
}

/** As tarefas, na ordem em que nasceram, depois de aplicar os eventos em ordem. */
export function tasksFrom(events: readonly EventDraft<TaskEvent>[]): TaskView[] {
  const tasks = new Map<string, TaskView>()
  for (const event of events) {
    if (event.type === 'urn:ads:event:task.created:1') {
      const { task_id: id, title } = event.data
      tasks.set(id, { id, state: 'pending', ...(title === undefined ? {} : { title }) })
      continue
    }
    const task = tasks.get(event.data.task_id)
    const next = TASK_STATE_AFTER[event.type]
    if (task && next) task.state = next
  }
  return [...tasks.values()]
}

/**
 * As sessões abertas, na ordem em que nasceram. Sessão sem `session.activity_changed` conta como
 * ativa; sessão encerrada sai da lista.
 */
export function sessionsFrom(events: readonly EventDraft<SessionEvent>[]): SessionView[] {
  const sessions = new Map<string, SessionView>()
  for (const event of events) {
    const id = event.data.session_id
    switch (event.type) {
      case 'urn:ads:event:session.created:1': {
        const name = event.data.display_name
        sessions.set(id, {
          id,
          activity: 'active',
          ...(name === undefined ? {} : { displayName: name }),
        })
        break
      }
      case 'urn:ads:event:session.activity_changed:1': {
        const session = sessions.get(id)
        if (session) session.activity = event.data.activity
        break
      }
      case 'urn:ads:event:session.ended:1':
        sessions.delete(id)
        break
      default:
        break
    }
  }
  return [...sessions.values()]
}
