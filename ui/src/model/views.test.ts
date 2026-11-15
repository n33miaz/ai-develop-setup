import { describe, expect, it } from 'vitest'
import type { EventDraft, SessionEvent, TaskEvent } from '../api'
import { createTranslator } from '../i18n'
import { sampleSessionEvents, sampleTaskEvents } from '../sample/data'
import { sessionsFrom, tasksFrom } from './views'

function taskCreated(taskId: string, title?: string): EventDraft<TaskEvent> {
  return {
    type: 'urn:ads:event:task.created:1',
    stream: `task/${taskId}`,
    data: {
      task_id: taskId,
      parent_task_id: null,
      project_id: 'project-1',
      objective_hash: `sha256:${'f'.repeat(64)}`,
      kind: 'sample',
      risk: 'low',
      oracle_declared: false,
      ...(title === undefined ? {} : { title }),
    },
  }
}

function sessionCreated(sessionId: string, displayName?: string): EventDraft<SessionEvent> {
  return {
    type: 'urn:ads:event:session.created:1',
    stream: `session/${sessionId}`,
    data: {
      session_id: sessionId,
      task_id: null,
      harness: 'harness-1',
      mode: 'interactive',
      cols: 80,
      rows: 24,
      workdir_alias: 'workdir-1',
      ...(displayName === undefined ? {} : { display_name: displayName }),
    },
  }
}

function activity(sessionId: string, value: 'active' | 'idle'): EventDraft<SessionEvent> {
  return {
    type: 'urn:ads:event:session.activity_changed:1',
    stream: `session/${sessionId}`,
    data: { session_id: sessionId, activity: value },
  }
}

describe('tasks from events', () => {
  it('derives the state and the title of the sample tasks', () => {
    const tasks = tasksFrom(sampleTaskEvents(createTranslator('en')))

    expect(tasks).toEqual([
      { id: 'task-1', title: 'Review the contracts module plan', state: 'running' },
      { id: 'task-2', title: 'Update the installation docs', state: 'pending' },
      { id: 'task-3', title: 'Check last night’s test report', state: 'done' },
    ])
  })

  it('keeps the title exactly as the event brings it', () => {
    const [task] = tasksFrom([taskCreated('task-1', '  <b>Título</b> com espaços  ')])

    expect(task?.title).toBe('  <b>Título</b> com espaços  ')
  })

  it('leaves the title out when the event has none', () => {
    const [task] = tasksFrom([taskCreated('task-1')])

    expect(task).toEqual({ id: 'task-1', state: 'pending' })
  })

  it('ignores events of a task that was never created', () => {
    const tasks = tasksFrom([
      {
        type: 'urn:ads:event:task.started:1',
        stream: 'task/ghost',
        data: { task_id: 'ghost', session_id: null },
      },
    ])

    expect(tasks).toEqual([])
  })
})

describe('sessions from events', () => {
  it('derives the name and the activity of the sample sessions', () => {
    const sessions = sessionsFrom(sampleSessionEvents(createTranslator('pt-BR')))

    expect(sessions).toEqual([
      { id: 'session-a', displayName: 'Sessão de exemplo A', activity: 'active' },
      { id: 'session-b', displayName: 'Sessão de exemplo B', activity: 'idle' },
      { id: 'session-c', displayName: 'Sessão de exemplo C', activity: 'idle' },
    ])
  })

  it('counts a session without activity events as active and leaves the name out', () => {
    const [session] = sessionsFrom([sessionCreated('session-1')])

    expect(session).toEqual({ id: 'session-1', activity: 'active' })
  })

  it('follows the last activity event of each session', () => {
    const sessions = sessionsFrom([
      sessionCreated('session-1', 'one'),
      sessionCreated('session-2', 'two'),
      activity('session-1', 'idle'),
      activity('session-2', 'idle'),
      activity('session-1', 'active'),
    ])

    expect(sessions.map((session) => [session.id, session.activity])).toEqual([
      ['session-1', 'active'],
      ['session-2', 'idle'],
    ])
  })

  it('ignores the activity of a session that was never created', () => {
    expect(sessionsFrom([activity('ghost', 'idle')])).toEqual([])
  })

  it('drops a session once it ends', () => {
    const sessions = sessionsFrom([
      sessionCreated('session-1'),
      sessionCreated('session-2'),
      {
        type: 'urn:ads:event:session.ended:1',
        stream: 'session/session-1',
        data: { session_id: 'session-1', reason: 'exit', exit_code: 0 },
      },
    ])

    expect(sessions.map((session) => session.id)).toEqual(['session-2'])
  })
})
