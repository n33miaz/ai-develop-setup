import {
  encodeBase64,
  type EventDraft,
  type EventSessionOutputV1,
  type EventTaskCreatedV1,
  type SessionEvent,
  type TaskEvent,
} from '../api'
import type { Translate } from '../i18n'

// Dados de exemplo locais: a tela ainda não lê nada do núcleo. Os fatos são eventos no formato dos
// contratos; os textos (título da tarefa, nome da sessão, saída do terminal) vêm do catálogo de
// i18n e viajam dentro dos eventos, como o texto do usuário viajará.

/** Os eventos que a tela reproduz enquanto o núcleo não a alimenta. */
export interface SampleEvents {
  tasks: readonly EventDraft<TaskEvent>[]
  sessions: readonly EventDraft<SessionEvent>[]
}

function taskCreated(
  taskId: string,
  hashDigit: string,
  title: string,
): EventDraft<EventTaskCreatedV1> {
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
      title,
    },
  }
}

export function sampleTaskEvents(t: Translate): EventDraft<TaskEvent>[] {
  return [
    taskCreated('task-1', 'a', t('sample.task.1')),
    {
      type: 'urn:ads:event:task.started:1',
      stream: 'task/task-1',
      data: { task_id: 'task-1', session_id: 'session-a' },
    },
    taskCreated('task-2', 'b', t('sample.task.2')),
    taskCreated('task-3', 'c', t('sample.task.3')),
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
}

function sessionCreated(
  sessionId: string,
  taskId: string | null,
  displayName: string,
): EventDraft<SessionEvent> {
  return {
    type: 'urn:ads:event:session.created:1',
    stream: `session/${sessionId}`,
    data: {
      session_id: sessionId,
      task_id: taskId,
      harness: 'sample-harness',
      mode: 'interactive',
      cols: 80,
      rows: 24,
      workdir_alias: 'sample-workdir',
      display_name: displayName,
    },
  }
}

function sessionIdle(sessionId: string): EventDraft<SessionEvent> {
  return {
    type: 'urn:ads:event:session.activity_changed:1',
    stream: `session/${sessionId}`,
    data: { session_id: sessionId, activity: 'idle' },
  }
}

// A sessão A fica sem evento de atividade: sessão recém-criada conta como ativa.
export function sampleSessionEvents(t: Translate): EventDraft<SessionEvent>[] {
  return [
    sessionCreated('session-a', 'task-1', t('sample.session.a')),
    sessionCreated('session-b', null, t('sample.session.b')),
    sessionIdle('session-b'),
    sessionCreated('session-c', null, t('sample.session.c')),
    sessionIdle('session-c'),
  ]
}

export function sampleEvents(t: Translate): SampleEvents {
  return { tasks: sampleTaskEvents(t), sessions: sampleSessionEvents(t) }
}

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
