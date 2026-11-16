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

  it('stays disconnected when disconnect is called before the connection completes', async () => {
    const client = createFakeCoreClient()
    const seen: ConnectionState[] = []
    client.onStateChange((state) => seen.push(state))

    const pending = client.connect()
    client.disconnect()

    await expect(pending).rejects.toMatchObject({ code: 'not_connected' })
    expect(client.getState()).toBe('disconnected')
    expect(seen).toEqual(['connecting', 'disconnected'])
  })

  it('connects on a new attempt made after a cancelled one', async () => {
    const client = createFakeCoreClient()
    const cancelled = client.connect()
    client.disconnect()

    const retry = client.connect()

    await expect(cancelled).rejects.toBeInstanceOf(CoreClientError)
    await expect(retry).resolves.toBeUndefined()
    expect(client.getState()).toBe('connected')
  })

  it('shares one attempt between calls made while connecting', async () => {
    const client = createFakeCoreClient()
    const seen: ConnectionState[] = []
    client.onStateChange((state) => seen.push(state))

    await Promise.all([client.connect(), client.connect()])

    expect(seen).toEqual(['connecting', 'connected'])
  })

  it('refuses to subscribe or send while disconnected', async () => {
    const client = createFakeCoreClient()

    expect(() => client.subscribe('session/a', 0, () => undefined)).toThrow(CoreClientError)
    await expect(client.send(newCommand('task.create'))).rejects.toMatchObject({
      code: 'not_connected',
    })
  })

  it('does not accept a command that was refused while disconnected', async () => {
    const client = createFakeCoreClient()
    const command = newCommand('task.create')
    await expect(client.send(command)).rejects.toBeInstanceOf(CoreClientError)

    await client.connect()
    const response = await client.send(command)

    expect(response.result).toEqual({ accepted: true, replayed: false })
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

  it('delivers nothing to a subscriber after the connection drops', async () => {
    const client = await connected()
    const seen: number[] = []
    client.subscribe('session/a', 0, (event) => seen.push(event.seq))
    client.emit(resized('a'))

    client.disconnect()
    client.emit(resized('a'))
    await client.connect()
    client.emit(resized('a'))

    expect(seen).toEqual([1])
  })

  it('stops a replay when the callback drops the connection', async () => {
    const client = await connected()
    for (const cols of [81, 82, 83]) client.emit(resized('a', cols))
    const seen: number[] = []

    client.subscribe('session/a', 0, (event) => {
      seen.push(event.seq)
      client.disconnect()
    })

    expect(client.getState()).toBe('disconnected')
    expect(seen).toEqual([1])
  })

  it('delivers nothing to a subscriber that another callback just removed', async () => {
    const client = await connected()
    const seen: number[] = []
    const stopOthers: (() => void)[] = []
    client.subscribe('session/a', 0, () => {
      for (const stop of stopOthers) stop()
    })
    stopOthers.push(client.subscribe('session/a', 0, (event) => seen.push(event.seq)))

    client.emit(resized('a'))

    expect(client.getState()).toBe('connected')
    expect(seen).toEqual([])
  })

  it('delivers to no other subscriber once one of them drops the connection', async () => {
    const client = await connected()
    const seen: number[] = []
    client.subscribe('session/a', 0, () => {
      client.disconnect()
    })
    client.subscribe('session/a', 0, (event) => seen.push(event.seq))

    client.emit(resized('a'))

    expect(client.getState()).toBe('disconnected')
    expect(seen).toEqual([])
  })

  it('does not revive a subscription when its callback reconnects', async () => {
    const client = await connected()
    for (const cols of [81, 82, 83]) client.emit(resized('a', cols))
    const seen: number[] = []

    client.subscribe('session/a', 0, (event) => {
      seen.push(event.seq)
      client.disconnect()
      void client.connect()
    })
    await client.connect()
    client.emit(resized('a', 84))

    expect(client.getState()).toBe('connected')
    expect(seen).toEqual([1])
  })

  it('keeps a recorded event out of reach of the draft and of other subscribers', async () => {
    const client = await connected()
    const draft = resized('a', 80)
    const recorded = client.emit(draft)
    draft.data.cols = 1

    const tamper = (event: EventEnvelopeV1): void => {
      Object.assign(event, { seq: 99 })
    }
    const tamperData = (event: EventEnvelopeV1): void => {
      Object.assign(event.data, { cols: 2 })
    }
    expect(() => client.subscribe('session/a', 0, tamper)).toThrow(TypeError)
    expect(() => client.subscribe('session/a', 0, tamperData)).toThrow(TypeError)
    const seen: EventEnvelopeV1[] = []
    client.subscribe('session/a', 0, (event) => seen.push(event))

    expect(seen).toEqual([recorded])
    expect(seen[0]).toMatchObject({ seq: 1, data: { session_id: 'a', cols: 80, rows: 24 } })
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
  it('accepts a new command and answers with the request id', async () => {
    const client = await connected()
    const command = newCommand('task.create', { title: 't' })

    const response = await client.send(command)

    expect(response).toEqual({
      jsonrpc: '2.0',
      id: command.id,
      result: { accepted: true, replayed: false },
    })
  })

  it('marks every resend of the same key as replayed', async () => {
    const client = await connected()
    const command = newCommand('task.create')
    const retry = { ...command, id: newCommand('task.create').id }

    const first = await client.send(command)
    const again = await client.send(command)
    const retried = await client.send(retry)

    expect(first.result).toEqual({ accepted: true, replayed: false })
    expect(again).toEqual({
      jsonrpc: '2.0',
      id: command.id,
      result: { accepted: true, replayed: true },
    })
    expect(retried).toEqual({
      jsonrpc: '2.0',
      id: retry.id,
      result: { accepted: true, replayed: true },
    })
  })

  it('keeps an answer already given when the command is replayed', async () => {
    const client = await connected()
    const command = newCommand('task.create')

    const first = await client.send(command)
    await client.send(command)

    expect(first.result).toEqual({ accepted: true, replayed: false })
  })

  it('accepts commands with different keys separately', async () => {
    const client = await connected()

    const first = await client.send(newCommand('task.create'))
    const second = await client.send(newCommand('task.create'))

    expect(first.result?.replayed).toBe(false)
    expect(second.result?.replayed).toBe(false)
  })
})
