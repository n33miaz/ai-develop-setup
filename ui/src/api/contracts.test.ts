import addFormats from 'ajv-formats'
import Ajv2020, { type AnySchema, type ValidateFunction } from 'ajv/dist/2020.js'
import { describe, expect, it } from 'vitest'
import {
  createFakeCoreClient,
  decodeBase64,
  newCommand,
  newIdempotencyKey,
  newUlid,
  type EventDraft,
} from '.'
import { streamBindingError } from '../../../contracts/tools/rules.mjs'
import { createTranslator, type Locale } from '../i18n'
import { sessionsFrom } from '../model/views'
import {
  sampleOutput,
  sampleSessionEvents,
  sampleTaskEvents,
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
const RESPONSE = 'urn:ads:command:response:2'

async function connected() {
  const client = createFakeCoreClient()
  await client.connect()
  return client
}

/** Tudo o que os dados de exemplo põem na tela, no idioma dado. */
function sampleDrafts(locale: Locale): EventDraft[] {
  const t = createTranslator(locale)
  const sessions = sampleSessionEvents(t)
  return [
    ...sampleTaskEvents(t),
    ...sessions,
    ...sessionsFrom(sessions).map((session) =>
      sampleOutput(t, session.id, session.displayName ?? ''),
    ),
  ]
}

function without(data: object, field: string): object {
  return Object.fromEntries(Object.entries(data).filter(([key]) => key !== field))
}

describe('messages against the contract schemas', () => {
  it('loads every contract schema', () => {
    expect(Object.keys(schemas).length).toBeGreaterThanOrEqual(55)
  })

  it.each(['pt-BR', 'en'] as const)(
    'emits sample task and session events that follow their schemas (%s)',
    async (locale) => {
      const client = await connected()
      const drafts = sampleDrafts(locale)
      expect(drafts).toHaveLength(14)

      const events = drafts.map((draft) => client.emit(draft))

      for (const event of events) {
        expect(violations(ENVELOPE, event), `${event.type} envelope`).toEqual([])
        expect(violations(event.type, event), event.type).toEqual([])
        expect(streamBindingError(event), event.type).toBeNull()
      }
      expect(new Set(events.map((event) => event.type))).toEqual(
        new Set([
          'urn:ads:event:task.created:1',
          'urn:ads:event:task.started:1',
          'urn:ads:event:task.completed:1',
          'urn:ads:event:session.created:1',
          'urn:ads:event:session.activity_changed:1',
          'urn:ads:event:session.output:1',
        ]),
      )
      expect(events.filter((event) => 'title' in event.data)).toHaveLength(3)
      expect(events.filter((event) => 'display_name' in event.data)).toHaveLength(3)
    },
  )

  it('accepts both activities and rejects an unknown one', async () => {
    const client = await connected()
    const idle = client.emit({
      type: 'urn:ads:event:session.activity_changed:1',
      stream: 'session/session-a',
      data: { session_id: 'session-a', activity: 'idle' },
    })
    const active = client.emit({
      type: 'urn:ads:event:session.activity_changed:1',
      stream: 'session/session-a',
      data: { session_id: 'session-a', activity: 'active' },
    })

    expect(violations(idle.type, idle)).toEqual([])
    expect(violations(active.type, active)).toEqual([])
    expect(
      violations(idle.type, { ...idle, data: { session_id: 'session-a', activity: 'paused' } }),
    ).toContain('enum@/data/activity')
  })

  it.each([
    ['title', 'urn:ads:event:task.created:1'],
    ['display_name', 'urn:ads:event:session.created:1'],
  ])('never classifies an event that carries %s as public', async (field, type) => {
    const client = await connected()
    const draft = sampleDrafts('pt-BR').find((item) => item.type === type)
    if (!draft) throw new Error(`missing sample ${type}`)
    const event = client.emit(draft)
    expect(event.data).toHaveProperty(field)

    expect(event.data_class).not.toBe('public')
    for (const dataClass of ['internal', 'private', 'secret']) {
      expect(violations(type, { ...event, data_class: dataClass }), dataClass).toEqual([])
    }
    expect(violations(type, { ...event, data_class: 'public' })).toContain('enum@/data_class')
    // Sem o texto do usuário o mesmo evento pode ser público: a regra é do campo, não do tipo.
    expect(
      violations(type, { ...event, data_class: 'public', data: without(event.data, field) }),
    ).toEqual([])
  })

  it('rejects user text that is empty, too long or has a line break', async () => {
    const client = await connected()
    const [created] = sampleTaskEvents(createTranslator('en'))
    if (!created) throw new Error('missing sample task')
    const event = client.emit(created)
    const titled = (title: string) => ({ ...event, data: { ...event.data, title } })

    expect(violations(event.type, titled(''))).toContain('minLength@/data/title')
    expect(violations(event.type, titled('x'.repeat(121)))).toContain('maxLength@/data/title')
    expect(violations(event.type, titled('one\ntwo'))).toContain('pattern@/data/title')
    expect(violations(event.type, titled('x'.repeat(120)))).toEqual([])
  })

  it('tells when the stream is not the one of the entity named in data', async () => {
    const client = await connected()
    const event = client.emit({
      type: 'urn:ads:event:session.activity_changed:1',
      stream: 'session/session-b',
      data: { session_id: 'session-a', activity: 'idle' },
    })

    // O Schema fixa só a família do stream; a igualdade com o id é regra de contrato à parte.
    expect(violations(event.type, event)).toEqual([])
    expect(streamBindingError(event)).toContain('data.session_id')
    expect(streamBindingError({ ...event, stream: 'session/session-a' })).toBeNull()
    expect(streamBindingError({ ...event, stream: 'task/session-a' })).toContain('data.task_id')
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

    for (const event of events) {
      expect(violations(event.type, event), event.type).toEqual([])
      expect(streamBindingError(event), event.type).toBeNull()
    }
  })

  it('builds commands and answers that follow the command schemas', async () => {
    const client = await connected()
    const command = newCommand('session.write', { session_id: 'session-a' })

    const response = await client.send(command)
    const repeated = await client.send(command)

    expect(violations(REQUEST, command)).toEqual([])
    expect(violations(RESPONSE, response)).toEqual([])
    expect(violations(RESPONSE, repeated)).toEqual([])
    expect(response.result).toEqual({ accepted: true, replayed: false })
    expect(repeated.result).toEqual({ accepted: true, replayed: true })
  })

  it('rejects an answer without the replay flag or with an open result', async () => {
    const client = await connected()
    const response = await client.send(newCommand('session.write', { session_id: 'session-a' }))

    expect(violations(RESPONSE, { ...response, result: { accepted: true } })).toContain(
      'required@/result',
    )
    expect(
      violations(RESPONSE, { ...response, result: { accepted: false, replayed: false } }),
    ).toContain('const@/result/accepted')
    expect(
      violations(RESPONSE, { ...response, result: { accepted: true, replayed: false, seq: 1 } }),
    ).toContain('additionalProperties@/result')
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
  it('carries the terminal text as the bytes of a session output event', () => {
    const t = createTranslator('en')
    const text = sampleTerminalText(t, 'núcleo')

    const { data } = sampleOutput(t, 'session-a', 'núcleo')
    const bytes = decodeBase64(data.payload)

    expect(new TextDecoder().decode(bytes)).toBe(text)
    expect(data.length).toBe(bytes.length)
  })
})
