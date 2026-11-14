import { describe, expect, it } from 'vitest'
import { CoreClientError, createFakeCoreClient, type ConnectionState, type CoreEvent } from '.'

async function connected() {
  const client = createFakeCoreClient()
  await client.connect()
  return client
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

    expect(() => client.subscribe('s', 0, () => undefined)).toThrow(CoreClientError)
    await expect(client.send({ method: 'task.create' }, 'k1')).rejects.toMatchObject({
      code: 'not_connected',
    })
  })
})

describe('fake core client: event replay', () => {
  it('numbers events per stream starting at 1', async () => {
    const client = await connected()

    const first = client.emit('a', 'x')
    const other = client.emit('b', 'x')
    const second = client.emit('a', 'x')

    expect([first.seq, second.seq, other.seq]).toEqual([1, 2, 1])
  })

  it('replays in order only the events after the given sequence number', async () => {
    const client = await connected()
    for (const n of [1, 2, 3, 4, 5]) client.emit('session', 'output', n)
    client.emit('other', 'output', 99)
    const seen: CoreEvent[] = []

    client.subscribe('session', 2, (event) => seen.push(event))

    expect(seen.map((event) => event.seq)).toEqual([3, 4, 5])
    expect(seen.map((event) => event.data)).toEqual([3, 4, 5])
  })

  it('continues with new events after the replay, without gaps or repeats', async () => {
    const client = await connected()
    client.emit('session', 'output')
    client.emit('session', 'output')
    const seen: number[] = []
    client.subscribe('session', 0, (event) => seen.push(event.seq))

    client.emit('session', 'output')
    client.emit('other', 'output')
    client.emit('session', 'output')

    expect(seen).toEqual([1, 2, 3, 4])
  })

  it('resumes from the last seen sequence number after a reconnect', async () => {
    const client = await connected()
    const seen: number[] = []
    const stop = client.subscribe('session', 0, (event) => seen.push(event.seq))
    client.emit('session', 'output')
    stop()
    client.disconnect()
    client.emit('session', 'output')
    client.emit('session', 'output')

    await client.connect()
    client.subscribe('session', seen.at(-1) ?? 0, (event) => seen.push(event.seq))

    expect(seen).toEqual([1, 2, 3])
  })

  it('stops delivering after unsubscribe', async () => {
    const client = await connected()
    const seen: number[] = []
    const stop = client.subscribe('session', 0, (event) => seen.push(event.seq))
    stop()

    client.emit('session', 'output')

    expect(seen).toEqual([])
  })
})

describe('fake core client: idempotent commands', () => {
  it('applies a command once and acknowledges it', async () => {
    const client = await connected()
    const seen: CoreEvent[] = []
    client.subscribe('commands', 0, (event) => seen.push(event))

    const ack = await client.send({ method: 'task.create', params: { title: 't' } }, 'k1')

    expect(ack).toEqual({ idempotencyKey: 'k1', duplicate: false, stream: 'commands', seq: 1 })
    expect(seen).toHaveLength(1)
    expect(seen[0]).toMatchObject({
      type: 'command.task.create',
      cause: 'k1',
      data: { title: 't' },
    })
  })

  it('does not repeat the effect when the same key is sent again', async () => {
    const client = await connected()
    const seen: CoreEvent[] = []
    client.subscribe('commands', 0, (event) => seen.push(event))

    const first = await client.send({ method: 'task.create' }, 'k1')
    const again = await client.send({ method: 'task.create' }, 'k1')

    expect(seen).toHaveLength(1)
    expect(again).toEqual({ ...first, duplicate: true })
  })

  it('applies commands with different keys separately', async () => {
    const client = await connected()
    const seen: CoreEvent[] = []
    client.subscribe('commands', 0, (event) => seen.push(event))

    await client.send({ method: 'task.create' }, 'k1')
    const second = await client.send({ method: 'task.create' }, 'k2')

    expect(seen.map((event) => event.seq)).toEqual([1, 2])
    expect(second.duplicate).toBe(false)
  })
})
