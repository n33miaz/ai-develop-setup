import addFormats from 'ajv-formats'
import Ajv2020, { type AnySchema, type ValidateFunction } from 'ajv/dist/2020.js'
import { describe, expect, it } from 'vitest'
import { createFakeCoreClient, decodeBase64, newCommand, newIdempotencyKey, newUlid } from '.'
import { createTranslator } from '../i18n'
import {
  sampleOutput,
  sampleSessionEvents,
  sampleSessions,
  sampleTaskEvents,
  sampleTasks,
  sampleTerminalText,
} from '../sample/data'

// Os mesmos Schemas e as mesmas opções do validador de `contracts/tools/lib.mjs`.
const schemas = import.meta.glob<AnySchema>('../../../contracts/schemas/*.schema.json', {
  eager: true,
  import: 'default',
})
const ajv = new Ajv2020({
  strict: true,
  strictRequired: false,
  allowUnionTypes: true,
  allErrors: true,
})
addFormats(ajv)
for (const schema of Object.values(schemas)) ajv.addSchema(schema)

function validatorOf(urn: string): ValidateFunction {
  const validate = ajv.getSchema(urn)
  if (!validate) throw new Error(`missing schema ${urn}`)
  return validate
}

/** Motivos da reprovação; lista vazia quer dizer que a mensagem segue o contrato. */
function violations(urn: string, message: unknown): string[] {
  const validate = validatorOf(urn)
  if (validate(message)) return []
  return (validate.errors ?? []).map((error) => `${error.keyword}@${error.instancePath || '/'}`)
}

const ENVELOPE = 'urn:ads:event:envelope:1'
const REQUEST = 'urn:ads:command:request:1'
const RESPONSE = 'urn:ads:command:response:1'

async function connected() {
  const client = createFakeCoreClient()
  await client.connect()
  return client
}

describe('messages against the contract schemas', () => {
  it('loads every contract schema', () => {
    expect(Object.keys(schemas).length).toBeGreaterThanOrEqual(55)
  })

  it('emits sample task and session events that follow their schemas', async () => {
    const client = await connected()
    const t = createTranslator('pt-BR')
    const drafts = [
      ...sampleTaskEvents,
      ...sampleSessionEvents,
      ...sampleSessions.map((session) => sampleOutput(t, session.id, t(session.nameKey))),
    ]
    expect(drafts).toHaveLength(12)

    for (const draft of drafts) {
      const event = client.emit(draft)

      expect(violations(ENVELOPE, event), `${event.type} envelope`).toEqual([])
      expect(violations(event.type, event), event.type).toEqual([])
    }
  })

  it('emits events of the other families the interface imports', async () => {
    const client = await connected()
    const hash = `sha256:${'d'.repeat(64)}`

    const events = [
      client.emit({
        type: 'urn:ads:event:session.resized:1',
        stream: 'session/session-a',
        data: { session_id: 'session-a', cols: 120, rows: 40 },
      }),
      client.emit({
        type: 'urn:ads:event:session.ended:1',
        stream: 'session/session-a',
        data: { session_id: 'session-a', reason: 'exit', exit_code: 0 },
      }),
      client.emit({
        type: 'urn:ads:event:task.failed:1',
        stream: 'task/task-2',
        data: { task_id: 'task-2', error_code: 'E_TIMEOUT', retriable: true },
      }),
      client.emit({
        type: 'urn:ads:event:task.cancelled:1',
        stream: 'task/task-2',
        data: { task_id: 'task-2', reason: 'owner' },
      }),
      client.emit({
        type: 'urn:ads:event:approval.requested:1',
        stream: 'approval/approval-1',
        data: {
          approval_id: 'approval-1',
          plan_hash: hash,
          risk: 'medium',
          summary: 'sample',
          expires_at: '2030-01-02T03:04:05Z',
          needs_elevation: false,
        },
      }),
      client.emit({
        type: 'urn:ads:event:approval.granted:1',
        stream: 'approval/approval-1',
        data: { approval_id: 'approval-1', plan_hash: hash, via: 'local_ui' },
      }),
    ]

    for (const event of events) expect(violations(event.type, event), event.type).toEqual([])
  })

  it('builds commands and answers that follow the command schemas', async () => {
    const client = await connected()
    const command = newCommand('session.write', { session_id: 'session-a' })

    const response = await client.send(command)
    const repeated = await client.send(command)

    expect(violations(REQUEST, command)).toEqual([])
    expect(violations(RESPONSE, response)).toEqual([])
    expect(violations(RESPONSE, repeated)).toEqual([])
  })

  it('generates identifiers in the contract format', () => {
    const ulid = validatorOf('urn:ads:common:ulid:1')

    expect(ulid(newUlid())).toBe(true)
    expect(ulid(newUlid(0))).toBe(true)
    expect(ulid(newUlid(2 ** 48 - 1))).toBe(true)
    expect(ulid(newIdempotencyKey())).toBe(true)
    expect(newUlid(1).slice(0, 10)).toBe('0000000001')
    expect(newUlid()).not.toBe(newUlid())
  })

  it('rejects a message that breaks the contract', async () => {
    const client = await connected()
    const event = client.emit({
      type: 'urn:ads:event:session.resized:1',
      stream: 'session/session-a',
      data: { session_id: 'session-a', cols: 120, rows: 40 },
    })

    expect(violations(event.type, { ...event, data: { session_id: 'session-a' } })).not.toEqual([])
    expect(violations(ENVELOPE, { ...event, id: 'not-a-ulid' })).toContain('pattern@/id')
    expect(violations(ENVELOPE, { ...event, extra: 1 })).toContain('additionalProperties@/')
    expect(
      violations(REQUEST, { jsonrpc: '2.0', id: event.id, method: 'x', params: {} }),
    ).toContain('required@/params')
  })
})

describe('sample data', () => {
  it('derives the task states from the task events', () => {
    expect(sampleTasks.map((task) => [task.id, task.state])).toEqual([
      ['task-1', 'running'],
      ['task-2', 'pending'],
      ['task-3', 'done'],
    ])
  })

  it('carries the terminal text as the bytes of a session output event', () => {
    const t = createTranslator('en')
    const text = sampleTerminalText(t, 'núcleo')

    const { data } = sampleOutput(t, 'session-a', 'núcleo')
    const bytes = decodeBase64(data.payload)

    expect(new TextDecoder().decode(bytes)).toBe(text)
    expect(data.length).toBe(bytes.length)
  })
})
