import { describe, expect, it } from 'vitest'
import {
  createFakeCoreClient,
  type ApprovalEvent,
  type EventDraft,
  type KnownEvent,
  type SessionEvent,
  type TaskEvent,
} from '../api'
import { createTranslator } from '../i18n'
import { sampleSessionEvents, sampleTaskEvents } from '../sample/data'
import { approvalsFrom, isApprovalEvent, sessionsFrom, tasksFrom } from './views'

const PLAN = `sha256:${'a'.repeat(64)}`
const OTHER_PLAN = `sha256:${'b'.repeat(64)}`

function requested(approvalId: string, actionId: string, plan = PLAN): EventDraft<ApprovalEvent> {
  return {
    type: 'urn:ads:event:approval.requested:2',
    stream: `approval/${approvalId}`,
    data: {
      approval_id: approvalId,
      action_id: actionId,
      plan_hash: plan,
      risk: 'high',
      summary: 'sample',
      expires_at: '2030-01-02T03:04:05Z',
      needs_elevation: false,
    },
  }
}

function granted(approvalId: string, actionId: string, plan = PLAN): EventDraft<ApprovalEvent> {
  return {
    type: 'urn:ads:event:approval.granted:2',
    stream: `approval/${approvalId}`,
    data: { approval_id: approvalId, action_id: actionId, plan_hash: plan, via: 'local_ui' },
  }
}

function denied(approvalId: string, actionId: string, plan = PLAN): EventDraft<ApprovalEvent> {
  return {
    type: 'urn:ads:event:approval.denied:2',
    stream: `approval/${approvalId}`,
    data: { approval_id: approvalId, action_id: actionId, plan_hash: plan, reason: null },
  }
}

function expired(approvalId: string, actionId: string, plan = PLAN): EventDraft<ApprovalEvent> {
  return {
    type: 'urn:ads:event:approval.expired:2',
    stream: `approval/${approvalId}`,
    data: { approval_id: approvalId, action_id: actionId, plan_hash: plan },
  }
}

/** O mesmo evento com outro `type`: a forma de um evento que a interface não conhece. */
function retyped(event: EventDraft, type: string): EventDraft {
  return { ...event, type }
}

/** A aprovação `:1`, como ficou num histórico antigo: sem `action_id`. */
function version1(event: EventDraft<ApprovalEvent>): EventDraft {
  const { action_id, ...data } = event.data
  expect(action_id).not.toBe('')
  return { ...event, type: event.type.replace(/:2$/, ':1'), data }
}

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

  it.each([
    [
      'failed',
      {
        type: 'urn:ads:event:task.failed:1',
        stream: 'task/task-1',
        data: { task_id: 'task-1', error_code: 'E_TIMEOUT', retriable: false },
      },
    ],
    [
      'cancelled',
      {
        type: 'urn:ads:event:task.cancelled:1',
        stream: 'task/task-1',
        data: { task_id: 'task-1', reason: 'owner' },
      },
    ],
  ] as const)('ends a task as %s, started or not', (state, last) => {
    const started: EventDraft<TaskEvent> = {
      type: 'urn:ads:event:task.started:1',
      stream: 'task/task-1',
      data: { task_id: 'task-1', session_id: null },
    }

    const [afterStart] = tasksFrom([taskCreated('task-1'), started, last])
    const [beforeStart] = tasksFrom([taskCreated('task-1'), last])

    expect(afterStart?.state).toBe(state)
    expect(beforeStart?.state).toBe(state)
  })

  it('does not let the end of one task change another', () => {
    const tasks = tasksFrom([
      taskCreated('task-1'),
      taskCreated('task-2'),
      {
        type: 'urn:ads:event:task.failed:1',
        stream: 'task/task-2',
        data: { task_id: 'task-2', error_code: 'E_TIMEOUT', retriable: true },
      },
    ])

    expect(tasks.map((task) => task.state)).toEqual(['pending', 'failed'])
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

describe('approvals from events', () => {
  it('keeps the action of the request and waits for the decision', () => {
    expect(approvalsFrom([requested('approval-1', 'action-1')])).toEqual([
      { id: 'approval-1', actionId: 'action-1', planHash: PLAN, state: 'pending' },
    ])
  })

  it.each([
    ['granted', granted],
    ['denied', denied],
    ['expired', expired],
  ] as const)('ends a requested approval as %s', (state, decision) => {
    const approvals = approvalsFrom([
      requested('approval-1', 'action-1'),
      decision('approval-1', 'action-1'),
    ])

    expect(approvals).toEqual([{ id: 'approval-1', actionId: 'action-1', planHash: PLAN, state }])
  })

  it('tells apart two actions that share the same plan', () => {
    const approvals = approvalsFrom([
      requested('approval-1', 'action-1'),
      requested('approval-2', 'action-2'),
      granted('approval-2', 'action-2'),
    ])

    expect(approvals).toEqual([
      { id: 'approval-1', actionId: 'action-1', planHash: PLAN, state: 'pending' },
      { id: 'approval-2', actionId: 'action-2', planHash: PLAN, state: 'granted' },
    ])
  })

  it.each([
    ['of another action', granted('approval-1', 'action-2')],
    ['of another plan', granted('approval-1', 'action-1', OTHER_PLAN)],
    [
      'in the stream of another approval',
      { ...granted('approval-1', 'action-1'), stream: 'approval/approval-2' },
    ],
    ['of an approval never requested', granted('approval-9', 'action-1')],
  ])('ignores a decision %s', (_, decision) => {
    const approvals = approvalsFrom([requested('approval-1', 'action-1'), decision])

    expect(approvals.map((approval) => approval.state)).toEqual(['pending'])
  })

  it('keeps the first request and the first decision', () => {
    const approvals = approvalsFrom([
      requested('approval-1', 'action-1'),
      requested('approval-1', 'action-2', OTHER_PLAN),
      denied('approval-1', 'action-1'),
      granted('approval-1', 'action-1'),
      expired('approval-1', 'action-1'),
    ])

    expect(approvals).toEqual([
      { id: 'approval-1', actionId: 'action-1', planHash: PLAN, state: 'denied' },
    ])
  })

  it('reads version 1 approvals as history that creates and decides nothing', () => {
    const history = [
      version1(requested('approval-0', 'action-0')),
      version1(granted('approval-0', 'action-0')),
    ]
    const mixed = [
      ...history,
      requested('approval-1', 'action-1'),
      version1(granted('approval-1', 'action-1')),
      version1(denied('approval-1', 'action-1')),
      version1(expired('approval-1', 'action-1')),
    ]

    expect(history.map((event) => isApprovalEvent(event))).toEqual([false, false])
    expect(approvalsFrom(history)).toEqual([])
    expect(approvalsFrom(mixed)).toEqual([
      { id: 'approval-1', actionId: 'action-1', planHash: PLAN, state: 'pending' },
    ])
  })

  it('never takes a version 2 approval that does not name its action', () => {
    const unnamed = {
      ...version1(requested('approval-1', 'action-1')),
      type: requested('x', 'y').type,
    }
    const blank = requested('approval-2', '')

    expect(approvalsFrom([unnamed, blank, granted('approval-1', 'action-1')])).toEqual([])
  })

  it.each([
    'urn:ads:event:approval.granted:3',
    'urn:ads:event:approval.granted',
    'urn:ads:event:approval.revoked:2',
    'urn:ads:event:routing.decided:1',
    'approval.granted:2',
    'toString',
  ])('leaves an event of the unknown type %s as an envelope', async (type) => {
    const unknown = retyped(granted('approval-1', 'action-1'), type)
    const client = createFakeCoreClient()
    await client.connect()
    const seen: string[] = []
    client.subscribe('approval/approval-1', 0, (event) => seen.push(event.type))

    client.emit(requested('approval-1', 'action-1'))
    const recorded = client.emit(unknown)

    expect(isApprovalEvent(unknown)).toBe(false)
    expect(approvalsFrom([requested('approval-1', 'action-1'), unknown])).toEqual([
      { id: 'approval-1', actionId: 'action-1', planHash: PLAN, state: 'pending' },
    ])
    // O evento não reconhecido chega inteiro a quem assina, como envelope.
    expect(seen).toEqual(['urn:ads:event:approval.requested:2', type])
    expect(recorded).toMatchObject({ type, seq: 2, data: unknown.data })
  })

  it('counts the four version 2 approvals among the events the interface knows', () => {
    const known: KnownEvent['type'][] = [
      'urn:ads:event:approval.requested:2',
      'urn:ads:event:approval.granted:2',
      'urn:ads:event:approval.denied:2',
      'urn:ads:event:approval.expired:2',
    ]
    // @ts-expect-error a `:1` não diz de qual ação é e saiu da união.
    const old: KnownEvent['type'] = 'urn:ads:event:approval.granted:1'

    expect(known.every((type) => isApprovalEvent({ type, stream: 'approval/a', data: {} }))).toBe(
      true,
    )
    expect(isApprovalEvent({ type: old, stream: 'approval/a', data: {} })).toBe(false)
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
