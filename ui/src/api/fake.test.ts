import { describe, expect, it } from 'vitest'
import {
  CoreClientError,
  createFakeCoreClient,
  newCommand,
  type ConnectionState,
  type EventDraft,
  type EventEnvelopeV1,
  type EventSessionResizedV1,
} from '.'

async function connected() {
  const client = createFakeCoreClient()
  await client.connect()
  return client
}

function resized(sessionId: string, cols = 80): EventDraft<EventSessionResizedV1> {
  return {
    type: 'urn:ads:event:session.resized:1',
    stream: `session/${sessionId}`,
    data: { session_id: sessionId, cols, rows: 24 },
  }
}

describe('fake core client: connection', () => {
  it('starts disconnected and reports each state change in order', async () => {
    const client = createFakeCoreClient()
    const seen: ConnectionState[] = []
    client.onStateChange((state) => seen.push(state))
    expect(client.getState()).toBe('disconnected')

    await client.connect()
    client.disconnect()

    expect(seen).toEqual(['connecting', 'connected', 'disconnected'])
  })

  it('stops notifying a listener after it unsubscribes', async () => {
    const client = createFakeCoreClient()
    const seen: ConnectionState[] = []
    const stop = client.onStateChange((state) => seen.push(state))
    stop()

    await client.connect()

    expect(seen).toEqual([])
  })

  it('refuses to subscribe or send while disconnected', async () => {
    const client = createFakeCoreClient()

    expect(() => client.subscribe('session/a', 0, () => undefined)).toThrow(CoreClientError)
    await expect(client.send(newCommand('task.create'))).rejects.toMatchObject({
      code: 'not_connected',
    })
    expect(client.applied()).toEqual([])
  })
})

describe('fake core client: event replay', () => {
  it('numbers events per stream starting at 1', async () => {
    const client = await connected()

    const first = client.emit(resized('a'))
    const other = client.emit(resized('b'))
    const second = client.emit(resized('a'))

    expect([first.seq, second.seq, other.seq]).toEqual([1, 2, 1])
  })

  it('fills the envelope around the draft and keeps what the draft sets', async () => {
    const client = await connected()

    const plain = client.emit(resized('a'))
    const custom = client.emit({
      ...resized('a'),
      actor: { kind: 'device', id: 'dev-1' },
      cause: plain.id,
      data_class: 'private',
    })

    expect(plain).toMatchObject({
      type: 'urn:ads:event:session.resized:1',
      stream: 'session/a',
      actor: { kind: 'daemon', id: 'fake-core' },
      cause: null,
      data_class: 'internal',
      data: { session_id: 'a', cols: 80, rows: 24 },
    })
    expect(custom).toMatchObject({
      actor: { kind: 'device', id: 'dev-1' },
      cause: plain.id,
      data_class: 'private',
    })
    expect(custom.id).not.toBe(plain.id)
  })

  it('replays in order only the events after the given sequence number', async () => {
    const client = await connected()
    for (const cols of [81, 82, 83, 84, 85]) client.emit(resized('a', cols))
    client.emit(resized('other', 99))
    const seen: EventEnvelopeV1[] = []

    client.subscribe('session/a', 2, (event) => seen.push(event))

    expect(seen.map((event) => event.seq)).toEqual([3, 4, 5])
    expect(seen.map((event) => event.data)).toMatchObject([
      { cols: 83 },
      { cols: 84 },
      { cols: 85 },
    ])
  })

  it('continues with new events after the replay, without gaps or repeats', async () => {
    const client = await connected()
    client.emit(resized('a'))
    client.emit(resized('a'))
    const seen: number[] = []
    client.subscribe('session/a', 0, (event) => seen.push(event.seq))

    client.emit(resized('a'))
    client.emit(resized('other'))
    client.emit(resized('a'))

    expect(seen).toEqual([1, 2, 3, 4])
  })

  it('resumes from the last seen sequence number after a reconnect', async () => {
    const client = await connected()
    const seen: number[] = []
    const stop = client.subscribe('session/a', 0, (event) => seen.push(event.seq))
    client.emit(resized('a'))
    stop()
    client.disconnect()
    client.emit(resized('a'))
    client.emit(resized('a'))

    await client.connect()
    client.subscribe('session/a', seen.at(-1) ?? 0, (event) => seen.push(event.seq))

    expect(seen).toEqual([1, 2, 3])
  })

  it('stops delivering after unsubscribe', async () => {
    const client = await connected()
    const seen: number[] = []
    const stop = client.subscribe('session/a', 0, (event) => seen.push(event.seq))
    stop()

    client.emit(resized('a'))

    expect(seen).toEqual([])
  })
})

describe('fake core client: idempotent commands', () => {
  it('applies a command once and answers with the request id', async () => {
    const client = await connected()
    const command = newCommand('task.create', { title: 't' })

    const response = await client.send(command)

    expect(response).toEqual({ jsonrpc: '2.0', id: command.id, result: { accepted: true } })
    expect(client.applied()).toEqual([command])
  })

  it('does not repeat the effect when the same key is sent again', async () => {
    const client = await connected()
    const command = newCommand('task.create')

    const first = await client.send(command)
    const again = await client.send(command)
    const retried = await client.send({ ...command, id: newCommand('task.create').id })

    expect(client.applied()).toEqual([command])
    expect(again).toEqual(first)
    expect(retried.result).toEqual(first.result)
  })

  it('applies commands with different keys separately', async () => {
    const client = await connected()
    const first = newCommand('task.create')
    const second = newCommand('task.create')

    await client.send(first)
    await client.send(second)

    expect(client.applied()).toEqual([first, second])
  })
})
