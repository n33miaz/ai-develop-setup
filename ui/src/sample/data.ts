import type { MessageKey, Translate } from '../i18n'

// Dados de exemplo locais: a tela ainda não lê nada do núcleo. Os textos vêm do catálogo de i18n.

export type TaskState = 'pending' | 'running' | 'done'
export type SessionState = 'active' | 'idle'

export interface SampleTask {
  id: string
  titleKey: MessageKey
  state: TaskState
}

export interface SampleSession {
  id: string
  nameKey: MessageKey
  state: SessionState
}

export const sampleTasks: readonly SampleTask[] = [
  { id: 'task-1', titleKey: 'sample.task.1', state: 'running' },
  { id: 'task-2', titleKey: 'sample.task.2', state: 'pending' },
  { id: 'task-3', titleKey: 'sample.task.3', state: 'done' },
]

export const sampleSessions: readonly SampleSession[] = [
  { id: 'session-a', nameKey: 'sample.session.a', state: 'active' },
  { id: 'session-b', nameKey: 'sample.session.b', state: 'idle' },
  { id: 'session-c', nameKey: 'sample.session.c', state: 'idle' },
]

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
