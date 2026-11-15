import {
  encodeBase64,
  type EventDraft,
  type EventSessionCreatedV1,
  type EventSessionOutputV1,
  type EventTaskCreatedV1,
  type TaskEvent,
} from '../api'
import type { MessageKey, Translate } from '../i18n'

// Dados de exemplo locais: a tela ainda não lê nada do núcleo. Os fatos são eventos no formato dos
// contratos; os textos vêm do catálogo de i18n.

export type TaskState = 'pending' | 'running' | 'done'

// PROVISÓRIO: os contratos não têm evento de sessão parada nem de atividade; só criada e encerrada.
export type SessionState = 'active' | 'idle'

export interface SampleTask {
  id: string
  // PROVISÓRIO: o evento da tarefa leva só o hash do objetivo; falta de onde a tela lê o título.
  titleKey: MessageKey
  state: TaskState
}

export interface SampleSession {
  id: string
  // PROVISÓRIO: o evento de sessão criada não traz nome de exibição.
  nameKey: MessageKey
  state: SessionState
}

function taskCreated(taskId: string, hashDigit: string): EventDraft<EventTaskCreatedV1> {
  return {
    type: 'urn:ads:event:task.created:1',
    stream: `task/${taskId}`,
    data: {
      task_id: taskId,
      parent_task_id: null,
      project_id: 'sample-project',
      objective_hash: `sha256:${hashDigit.repeat(64)}`,
      kind: 'sample',
      risk: 'low',
      oracle_declared: false,
    },
  }
}

export const sampleTaskEvents: readonly EventDraft<TaskEvent>[] = [
  taskCreated('task-1', 'a'),
  {
    type: 'urn:ads:event:task.started:1',
    stream: 'task/task-1',
    data: { task_id: 'task-1', session_id: 'session-a' },
  },
  taskCreated('task-2', 'b'),
  taskCreated('task-3', 'c'),
  {
    type: 'urn:ads:event:task.started:1',
    stream: 'task/task-3',
    data: { task_id: 'task-3', session_id: null },
  },
  {
    type: 'urn:ads:event:task.completed:1',
    stream: 'task/task-3',
    data: { task_id: 'task-3', tests_passed: true, diff_sha: null },
  },
]

const TASK_STATE_AFTER: Partial<Record<TaskEvent['type'], TaskState>> = {
  'urn:ads:event:task.created:1': 'pending',
  'urn:ads:event:task.started:1': 'running',
  'urn:ads:event:task.completed:1': 'done',
}

/** Estado de cada tarefa depois de aplicar os eventos em ordem. */
export function taskStates(events: readonly EventDraft<TaskEvent>[]): Map<string, TaskState> {
  const states = new Map<string, TaskState>()
  for (const event of events) {
    const next = TASK_STATE_AFTER[event.type]
    if (next) states.set(event.data.task_id, next)
  }
  return states
}

const sampleTaskStates = taskStates(sampleTaskEvents)

const SAMPLE_TASK_TITLES: readonly Pick<SampleTask, 'id' | 'titleKey'>[] = [
  { id: 'task-1', titleKey: 'sample.task.1' },
  { id: 'task-2', titleKey: 'sample.task.2' },
  { id: 'task-3', titleKey: 'sample.task.3' },
]

export const sampleTasks: readonly SampleTask[] = SAMPLE_TASK_TITLES.map((task) => ({
  ...task,
  state: sampleTaskStates.get(task.id) ?? 'pending',
}))

export const sampleSessions: readonly SampleSession[] = [
  { id: 'session-a', nameKey: 'sample.session.a', state: 'active' },
  { id: 'session-b', nameKey: 'sample.session.b', state: 'idle' },
  { id: 'session-c', nameKey: 'sample.session.c', state: 'idle' },
]

export const sampleSessionEvents: readonly EventDraft<EventSessionCreatedV1>[] = sampleSessions.map(
  (session) => ({
    type: 'urn:ads:event:session.created:1',
    stream: `session/${session.id}`,
    data: {
      session_id: session.id,
      task_id: session.id === 'session-a' ? 'task-1' : null,
      harness: 'sample-harness',
      mode: 'interactive',
      cols: 80,
      rows: 24,
      workdir_alias: 'sample-workdir',
    },
  }),
)

const RESET = '\x1b[0m'
const BOLD = '\x1b[1m'
const MUTED = '\x1b[90m'
const RED = '\x1b[31m'
const YELLOW = '\x1b[33m'
const CYAN = '\x1b[36m'
const PROMPT = `${MUTED}$${RESET}`

/** Texto de exemplo do terminal, com cores ANSI, para a sessão dada. */
export function sampleTerminalText(t: Translate, sessionName: string): string {
  return [
    `${BOLD}${t('sample.terminal.title')}${RESET} - ${sessionName}`,
    `${PROMPT} ${t('sample.terminal.prompt')}`,
    `${RED}[x]${RESET} ${t('sample.terminal.core')}`,
    `${YELLOW}[!]${RESET} ${t('sample.terminal.data')}`,
    `${CYAN}[>]${RESET} ${t('sample.terminal.next')}`,
    `${PROMPT} `,
  ].join('\r\n')
}

/** O texto de exemplo como o registro de saída da sessão: bytes em base64, a partir do começo. */
export function sampleOutput(
  t: Translate,
  sessionId: string,
  sessionName: string,
): EventDraft<EventSessionOutputV1> {
  const bytes = new TextEncoder().encode(sampleTerminalText(t, sessionName))
  return {
    type: 'urn:ads:event:session.output:1',
    stream: `session/${sessionId}`,
    data_class: 'private',
    data: {
      session_id: sessionId,
      offset: 0,
      length: bytes.length,
      encoding: 'base64',
      payload: encodeBase64(bytes),
    },
  }
}
