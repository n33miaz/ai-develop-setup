import type {
  ApprovalEvent,
  EventDraft,
  EventSessionActivityChangedV1,
  SessionEvent,
  TaskEvent,
} from '../api'

// O que a barra lateral mostra de cada tarefa e de cada sessão, e o estado de cada aprovação,
// reconstruídos só dos eventos.

/** Estado local da tela: os contratos trazem os eventos da tarefa, não um estado pronto. */
export type TaskState = 'pending' | 'running' | 'done' | 'failed' | 'cancelled'

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

/** Estado local da tela: sai dos eventos da aprovação, não do registro `record.approval`. */
export type ApprovalState = 'pending' | 'granted' | 'denied' | 'expired'

export interface ApprovalView {
  id: string
  /** O `action_id` do pedido. É ele que diz de qual ação é a aprovação: o plano pode se repetir. */
  actionId: string
  planHash: string
  state: ApprovalState
}

// Um estado por tipo de evento: evento de tarefa novo nos contratos não compila sem entrar aqui.
const TASK_STATE_AFTER: Record<TaskEvent['type'], TaskState> = {
  'urn:ads:event:task.created:1': 'pending',
  'urn:ads:event:task.started:1': 'running',
  'urn:ads:event:task.completed:1': 'done',
  'urn:ads:event:task.failed:1': 'failed',
  'urn:ads:event:task.cancelled:1': 'cancelled',
}

// O mesmo para as aprovações. Só as `:2`: as `:1` não dizem de qual ação são e o núcleo as recusa.
const APPROVAL_STATE_AFTER: Record<ApprovalEvent['type'], ApprovalState> = {
  'urn:ads:event:approval.requested:2': 'pending',
  'urn:ads:event:approval.granted:2': 'granted',
  'urn:ads:event:approval.denied:2': 'denied',
  'urn:ads:event:approval.expired:2': 'expired',
}

/**
 * Diz se o evento é uma das aprovações que a interface conhece, pela URN inteira, versão incluída:
 * `approval.granted:1` não é `approval.granted:2`. O que não passa continua sendo só um envelope.
 */
export function isApprovalEvent(event: EventDraft): event is EventDraft<ApprovalEvent> {
  return Object.hasOwn(APPROVAL_STATE_AFTER, event.type)
}

/**
 * As aprovações, na ordem em que foram pedidas, depois de aplicar os eventos em ordem. Recebe
 * qualquer evento e ignora o que não é aprovação `:2`: uma `:1` no histórico não cria nem decide
 * nada. A decisão só vale para a aprovação pendente da mesma ação e do mesmo plano; a primeira
 * decisão fica.
 */
export function approvalsFrom(events: readonly EventDraft[]): ApprovalView[] {
  const approvals = new Map<string, ApprovalView>()
  for (const event of events) {
    if (!isApprovalEvent(event)) continue
    const { approval_id: id, action_id: actionId, plan_hash: planHash } = event.data
    // O tipo não é conferido em execução: sem a ação, ou em fluxo de outra entidade, não vale.
    if (typeof actionId !== 'string' || actionId === '') continue
    if (event.stream !== `approval/${id}`) continue
    const approval = approvals.get(id)
    if (event.type === 'urn:ads:event:approval.requested:2') {
      if (!approval) approvals.set(id, { id, actionId, planHash, state: 'pending' })
      continue
    }
    if (
      approval?.state === 'pending' &&
      approval.actionId === actionId &&
      approval.planHash === planHash
    ) {
      approval.state = APPROVAL_STATE_AFTER[event.type]
    }
  }
  return [...approvals.values()]
}

/** As tarefas, na ordem em que nasceram, depois de aplicar os eventos em ordem. */
export function tasksFrom(events: readonly EventDraft<TaskEvent>[]): TaskView[] {
  const tasks = new Map<string, TaskView>()
  for (const event of events) {
    if (event.type === 'urn:ads:event:task.created:1') {
      const { task_id: id, title } = event.data
      tasks.set(id, {
        id,
        state: TASK_STATE_AFTER[event.type],
        ...(title === undefined ? {} : { title }),
      })
      continue
    }
    const task = tasks.get(event.data.task_id)
    if (task) task.state = TASK_STATE_AFTER[event.type]
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
