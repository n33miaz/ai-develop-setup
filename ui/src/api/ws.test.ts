import Ajv2020, { type AnySchema } from 'ajv/dist/2020.js'
import addFormats from 'ajv-formats'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { HarnessListError } from './client'
import { newCommand } from './command'
import { createFakeCoreClient } from './fake'
import type { CommandRequestV1 } from './contracts'
import { createWsCoreClient } from './ws'

const schemas = import.meta.glob<AnySchema>('../../../contracts/schemas/*.schema.json', {
  eager: true,
  import: 'default',
})
const ajv = new Ajv2020({ strict: true, strictRequired: false, allowUnionTypes: true })
addFormats(ajv)
for (const schema of Object.values(schemas)) ajv.addSchema(schema)
function valid(urn: string, message: unknown) {
  const validate = ajv.getSchema(urn)
  if (!validate) throw new Error('missing schema')
  expect(validate(message), JSON.stringify(validate.errors)).toBe(true)
}

class FakeSocket extends EventTarget {
  sent: CommandRequestV1[] = []
  send(text: string) {
    const command = JSON.parse(text) as CommandRequestV1
    valid(`urn:ads:command:${command.method}:1`, command)
    this.sent.push(command)
  }
  close() {
    this.dispatchEvent(new Event('close'))
  }
  receive(message: unknown) {
    valid('urn:ads:contract:local-api.server-message:2', message)
    this.receiveRaw(message)
  }
  /** Entrega sem conferir: simula um núcleo que manda o que o contrato não aceita. */
  receiveRaw(message: unknown) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(message) }))
  }
  answer(index: number) {
    this.receive({
      jsonrpc: '2.0',
      id: this.sent[index]?.id,
      result: { accepted: true, replayed: false },
    })
  }
}
const token = 'a'.repeat(43)
function setup(listTimeoutMs?: number) {
  const socket = new FakeSocket()
  const urls: string[] = []
  const client = createWsCoreClient({
    url: 'ws://127.0.0.1:12345',
    token,
    ...(listTimeoutMs === undefined ? {} : { listTimeoutMs }),
    socketFactory: (url) => {
      urls.push(url)
      return socket
    },
  })
  return { socket, client, urls }
}
async function connected(listTimeoutMs?: number) {
  const context = setup(listTimeoutMs)
  const ready = context.client.connect()
  context.socket.dispatchEvent(new Event('open'))
  context.socket.answer(0)
  await ready
  return context
}
function event(seq: number, stream = 'session/session-a') {
  const fake = createFakeCoreClient()
  return {
    ...fake.emit({
      data_class: 'private',
      type: 'urn:ads:event:session.output:1',
      stream,
      data: {
        session_id: stream.split('/')[1],
        offset: 0,
        length: 1,
        payload: 'YQ==',
        encoding: 'base64',
      },
    }),
    seq,
  }
}

describe('local WebSocket client', () => {
  it('accepts the oldest retained event when replay starts from zero', async () => {
    const { client, socket } = await connected()
    const seen: number[] = []
    client.subscribe('session/session-a', 0, (event) => seen.push(event.seq))
    socket.answer(1)
    socket.receive(event(7))
    socket.receive(event(8))
    expect(seen).toEqual([7, 8])
    expect(client.getState()).toBe('connected')
  })
  it('validates every supported session command before sending it', async () => {
    const { client, socket } = await connected()
    const commands = [
      newCommand('session.open', {
        session_id: 'session-a',
        harness: 'harness-a',
        workdir_alias: 'workspace-a',
        cols: 80,
        rows: 24,
      }),
      newCommand('session.write', { session_id: 'session-a', data: 'YQ==' }),
    ]
    for (const command of commands) {
      const response = client.send(command)
      socket.answer(socket.sent.length - 1)
      expect((await response).id).toBe(command.id)
    }
    await expect(
      client.send(newCommand('session.write', { session_id: 'session-a' })),
    ).rejects.toThrow('invalid local command')
    await expect(client.send(newCommand('fs.delete', {}))).rejects.toThrow('invalid local command')
  })
  it('rejects malformed server messages without exposing their contents', async () => {
    const { client, socket } = await connected()
    const rejected = expect(
      client.send(newCommand('session.close', { session_id: 'session-a' })),
    ).rejects.toThrow('core disconnected')
    socket.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ token }) }))
    await rejected
    expect(client.getState()).toBe('disconnected')
  })
  it('disconnects on an invalid session payload even when its envelope is valid', async () => {
    const { client, socket } = await connected()
    const invalid = { ...event(1), data: { payload: token } }
    socket.receive(invalid)
    expect(client.getState()).toBe('disconnected')
  })
  it('does not deliver an event across a sequence gap', async () => {
    const { client, socket } = await connected()
    const seen: number[] = []
    client.subscribe('session/session-a', 0, (event) => seen.push(event.seq))
    socket.answer(1)
    socket.receive(event(1))
    socket.receive(event(3))
    expect(seen).toEqual([1])
    expect(client.getState()).toBe('disconnected')
  })
  it('ignores an old socket after a new connection starts', async () => {
    const first = new FakeSocket()
    const second = new FakeSocket()
    let attempts = 0
    const client = createWsCoreClient({
      url: 'ws://localhost:12345',
      token,
      socketFactory: () => (attempts++ === 0 ? first : second),
    })
    const rejected = expect(client.connect()).rejects.toThrow('core disconnected')
    client.disconnect()
    const ready = client.connect()
    await rejected
    expect(client.connect()).toBe(ready)
    first.dispatchEvent(new Event('open'))
    expect(first.sent).toHaveLength(0)
    second.dispatchEvent(new Event('open'))
    second.answer(0)
    await ready
    first.close()
    expect(client.getState()).toBe('connected')
  })
  it('authenticates before connecting or sending another command', async () => {
    const { client, socket, urls } = setup()
    const ready = client.connect()
    expect(client.connect()).toBe(ready)
    expect(client.getState()).toBe('connecting')
    await expect(
      client.send(newCommand('session.close', { session_id: 'session-a' })),
    ).rejects.toThrow('core disconnected')
    socket.dispatchEvent(new Event('open'))
    expect(socket.sent).toHaveLength(1)
    expect(socket.sent[0]).toMatchObject({ method: 'auth.hello', params: { token } })
    expect(urls).toEqual(['ws://127.0.0.1:12345/'])
    socket.answer(0)
    await ready
    expect(client.getState()).toBe('connected')
  })
  it('matches responses by id even when replies arrive in reverse order', async () => {
    const { client, socket } = await connected()
    const first = newCommand('session.close', { session_id: 'session-a' })
    const second = newCommand('session.resize', { session_id: 'session-a', cols: 80, rows: 24 })
    const a = client.send(first)
    const b = client.send(second)
    socket.answer(2)
    socket.answer(1)
    expect((await a).id).toBe(first.id)
    expect((await b).id).toBe(second.id)
  })
  it('subscribes from a cursor and delivers only the matching stream in sequence', async () => {
    const { client, socket } = await connected()
    const seen: number[] = []
    const stop = client.subscribe('session/session-a', 2, (event) => seen.push(event.seq))
    expect(socket.sent[1]).toMatchObject({
      method: 'events.subscribe',
      params: { stream: 'session/session-a', after_seq: 2 },
    })
    socket.answer(1)
    socket.receive(event(3, 'session/session-b'))
    socket.receive(event(2))
    socket.receive(event(3))
    socket.receive(event(3))
    socket.receive(event(4))
    stop()
    socket.receive(event(5))
    expect(seen).toEqual([3, 4])
  })
  it('ends only the subscription answered later with -32004 and keeps the connection', async () => {
    const { client, socket } = await connected()
    const seen: string[] = []
    const ended: number[] = []
    client.subscribe(
      'session/session-a',
      0,
      (event) => seen.push(event.stream),
      (code) => ended.push(code),
    )
    client.subscribe('session/session-b', 0, (event) => seen.push(event.stream))
    socket.answer(1)
    socket.answer(2)
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32004, message: 'replay unavailable' },
    })
    socket.receive(event(1))
    socket.receive(event(1, 'session/session-b'))
    expect(ended).toEqual([-32004])
    expect(seen).toEqual(['session/session-b'])
    expect(client.getState()).toBe('connected')
  })
  it('reports a pending close on subscribe without disconnecting', async () => {
    const { client, socket } = await connected()
    const ended: number[] = []
    client.subscribe(
      'session/session-a',
      0,
      () => undefined,
      (code) => ended.push(code),
    )
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32013, message: 'session close pending' },
    })
    await Promise.resolve()
    expect(ended).toEqual([-32013])
    expect(client.getState()).toBe('connected')
  })
  it('ignores the end of a subscription already cancelled', async () => {
    const { client, socket } = await connected()
    const ended: number[] = []
    const stop = client.subscribe(
      'session/session-a',
      0,
      () => undefined,
      (code) => ended.push(code),
    )
    socket.answer(1)
    stop()
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32004, message: 'replay unavailable' },
    })
    expect(ended).toEqual([])
    expect(client.getState()).toBe('connected')
  })
  it('ignores a late refusal of a subscription cancelled before its answer', async () => {
    const { client, socket } = await connected()
    const seen: string[] = []
    const stop = client.subscribe('session/session-a', 0, (event) => seen.push(event.stream))
    client.subscribe('session/session-b', 0, (event) => seen.push(event.stream))
    stop()
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32602, message: 'invalid command' },
    })
    await Promise.resolve()
    socket.answer(2)
    socket.receive(event(1, 'session/session-b'))
    expect(client.getState()).toBe('connected')
    expect(seen).toEqual(['session/session-b'])
  })
  it('disconnects on an unexpected error after a subscription was accepted', async () => {
    const { client, socket } = await connected()
    client.subscribe('session/session-a', 0, () => undefined)
    socket.answer(1)
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32603, message: 'service unavailable' },
    })
    expect(client.getState()).toBe('disconnected')
  })
  it('still disconnects when a subscription is refused for another reason', async () => {
    const { client, socket } = await connected()
    client.subscribe('session/session-a', 0, () => undefined)
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32602, message: 'invalid command' },
    })
    await Promise.resolve()
    expect(client.getState()).toBe('disconnected')
  })
  it('rejects pending requests and announces disconnection on transport failure', async () => {
    const { client, socket } = await connected()
    const states: string[] = []
    client.onStateChange((state) => states.push(state))
    const pending = client.send(
      newCommand('session.write', { session_id: 'session-a', data: 'YQ==' }),
    )
    const rejected = expect(pending).rejects.toThrow('core disconnected')
    socket.close()
    await rejected
    expect(client.getState()).toBe('disconnected')
    expect(states).toEqual(['disconnected'])
  })
  it('settles a connection cancelled before the socket opens', async () => {
    const { client } = setup()
    const rejected = expect(client.connect()).rejects.toThrow('core disconnected')
    client.disconnect()
    await rejected
  })
  it('does not expose server authentication errors', async () => {
    const { client, socket } = setup()
    const rejected = expect(client.connect()).rejects.toThrow('core disconnected')
    socket.dispatchEvent(new Event('open'))
    socket.receive({ jsonrpc: '2.0', id: socket.sent[0]?.id, error: { code: -1, message: token } })
    await rejected
  })
  it('lists the configured tools through harness.list', async () => {
    const { client, socket } = await connected()
    const listed = client.listHarnesses()
    expect(socket.sent[1]).toMatchObject({ method: 'harness.list', params: { _meta: {} } })
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      result: { harnesses: ['claude'], workdir_aliases: ['default'] },
    })
    await expect(listed).resolves.toEqual({ harnesses: ['claude'], workdir_aliases: ['default'] })
    await expect(client.send(newCommand('harness.list'))).rejects.toThrow('use listHarnesses')
  })
  it('rejects a refused harness.list without exposing the server message', async () => {
    const { client, socket } = await connected()
    const listed = client.listHarnesses()
    socket.receive({ jsonrpc: '2.0', id: socket.sent[1]?.id, error: { code: -1, message: token } })
    await expect(listed).rejects.toThrow('command rejected')
    expect(client.getState()).toBe('connected')
  })
  it('disconnects when a reply does not have the shape of its request', async () => {
    const list = await connected()
    const listed = list.client.listHarnesses()
    list.socket.answer(1)
    await expect(listed).rejects.toThrow('core disconnected')
    expect(list.client.getState()).toBe('disconnected')
    const other = await connected()
    const closing = other.client.send(newCommand('session.close', { session_id: 'session-a' }))
    other.socket.receive({
      jsonrpc: '2.0',
      id: other.socket.sent[1]?.id,
      result: { harnesses: [], workdir_aliases: [] },
    })
    await expect(closing).rejects.toThrow('core disconnected')
  })
  it.each([
    ['an extra field', { harnesses: ['claude'], workdir_aliases: ['default'], program: 'x' }],
    ['a POSIX path as alias', { harnesses: ['claude'], workdir_aliases: ['/srv/work'] }],
    ['a Windows path as alias', { harnesses: ['claude'], workdir_aliases: ['C:\\work'] }],
    ['a repeated tool', { harnesses: ['claude', 'claude'], workdir_aliases: [] }],
    ['no alias list', { harnesses: ['claude'] }],
  ])('drops the connection on a tool list with %s', async (_, result) => {
    const { client, socket } = await connected()
    const listed = client.listHarnesses()
    socket.receiveRaw({ jsonrpc: '2.0', id: socket.sent[1]?.id, result })
    await expect(listed).rejects.toThrow('core disconnected')
    expect(client.getState()).toBe('disconnected')
  })
  it('rejects the list when an older core does not know the method', async () => {
    const { client, socket } = await connected()
    const listed = client.listHarnesses()
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32601, message: 'unknown method' },
    })
    await expect(listed).rejects.toThrow('command rejected')
    expect(client.getState()).toBe('connected')
  })
  it('gives up on a list the core never answers and ignores the late reply', async () => {
    const { client, socket } = await connected(20)
    const listed = client.listHarnesses()
    await expect(listed).rejects.toThrow('core timeout')
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      result: { harnesses: ['claude'], workdir_aliases: ['default'] },
    })
    expect(client.getState()).toBe('connected')
    const again = client.listHarnesses()
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[2]?.id,
      result: { harnesses: [], workdir_aliases: [] },
    })
    await expect(again).resolves.toEqual({ harnesses: [], workdir_aliases: [] })
  })
  it('refuses to authenticate when the hello reply is a tool list', async () => {
    const { client, socket } = setup()
    const rejected = expect(client.connect()).rejects.toThrow('core disconnected')
    socket.dispatchEvent(new Event('open'))
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[0]?.id,
      result: { harnesses: [], workdir_aliases: [] },
    })
    await rejected
  })
  it.each([
    'ws://127.0.0.1:12345/?token=secret',
    'ws://example.test',
    'ws://user:secret@localhost:12345',
  ])('rejects an unsafe endpoint without echoing it', (url) => {
    expect(() => createWsCoreClient({ url, token })).toThrow('invalid local endpoint')
  })
})

describe('tool list deadline', () => {
  afterEach(() => {
    vi.useRealTimers()
  })
  it('tells a timeout apart from an older core that does not know the method', async () => {
    vi.useFakeTimers()
    const { client, socket } = await connected(50)
    const late = client.listHarnesses()
    const lateReason = late.catch((error: unknown) => (error as HarnessListError).reason)
    await vi.advanceTimersByTimeAsync(50)
    expect(await lateReason).toBe('timeout')
    const old = client.listHarnesses()
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[2]?.id,
      error: { code: -32601, message: 'unknown method' },
    })
    await expect(old).rejects.toMatchObject({ reason: 'unsupported' })
    const refused = client.listHarnesses()
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[3]?.id,
      error: { code: -32602, message: 'invalid command' },
    })
    await expect(refused).rejects.toMatchObject({ reason: 'refused' })
  })
  it('clears the deadline when the list arrives or the connection drops', async () => {
    vi.useFakeTimers()
    const { client, socket } = await connected(50)
    const listed = client.listHarnesses()
    expect(vi.getTimerCount()).toBe(1)
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      result: { harnesses: [], workdir_aliases: [] },
    })
    await listed
    expect(vi.getTimerCount()).toBe(0)
    const dropped = client.listHarnesses()
    socket.close()
    await expect(dropped).rejects.toThrow('core disconnected')
    expect(vi.getTimerCount()).toBe(0)
  })
  it('discards late replies to an expired list while other requests continue', async () => {
    vi.useFakeTimers()
    const { client, socket } = await connected(50)
    const late = client.listHarnesses()
    const settled = late.catch(() => undefined)
    await vi.advanceTimersByTimeAsync(50)
    await settled
    const closing = client.send(newCommand('session.close', { session_id: 'session-a' }))
    socket.receiveRaw({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      result: { harnesses: ['claude'], workdir_aliases: ['/srv/work'], program: 'x' },
    })
    expect(client.getState()).toBe('connected')
    socket.answer(2)
    await expect(closing).resolves.toMatchObject({ result: { accepted: true } })
    socket.receive({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      result: { harnesses: ['claude'], workdir_aliases: ['default'] },
    })
    expect(client.getState()).toBe('connected')
  })
})

describe('expired tool list ids', () => {
  afterEach(() => {
    vi.useRealTimers()
  })
  it('forgets expired ids when the connection ends', async () => {
    vi.useFakeTimers()
    const sockets: FakeSocket[] = []
    const client = createWsCoreClient({
      url: 'ws://127.0.0.1:12345',
      token,
      listTimeoutMs: 50,
      socketFactory: () => {
        const socket = new FakeSocket()
        sockets.push(socket)
        return socket
      },
    })
    const open = async () => {
      const ready = client.connect()
      const socket = sockets[sockets.length - 1]
      if (!socket) throw new Error('missing socket')
      socket.dispatchEvent(new Event('open'))
      socket.answer(0)
      await ready
      return socket
    }
    const first = await open()
    const late = client.listHarnesses().catch(() => undefined)
    await vi.advanceTimersByTimeAsync(50)
    await late
    const expiredId = first.sent[1]?.id
    client.disconnect()
    const second = await open()
    second.receiveRaw({
      jsonrpc: '2.0',
      id: expiredId,
      result: { harnesses: ['claude'], workdir_aliases: ['/srv/work'] },
    })
    expect(client.getState()).toBe('disconnected')
  })
  it('keeps only the newest 32 expired ids', async () => {
    vi.useFakeTimers()
    const { client, socket } = await connected(50)
    const lists = Array.from({ length: 33 }, () => client.listHarnesses().catch(() => undefined))
    await vi.advanceTimersByTimeAsync(50)
    await Promise.all(lists)
    const malformed = (id: string | undefined) => ({
      jsonrpc: '2.0',
      id,
      result: { harnesses: ['claude'], workdir_aliases: ['/srv/work'] },
    })
    socket.receiveRaw(malformed(socket.sent[33]?.id))
    expect(client.getState()).toBe('connected')
    socket.receiveRaw(malformed(socket.sent[1]?.id))
    expect(client.getState()).toBe('disconnected')
  })
})
