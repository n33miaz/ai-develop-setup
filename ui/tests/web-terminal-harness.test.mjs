import assert from 'node:assert/strict'
import { once } from 'node:events'
import test from 'node:test'
import { cdpRequests, stop, withTestProcesses } from './web-terminal-harness.mjs'

test('partial startup preserves the failure and reaps every started process', async (t) => {
  const started = []
  t.after(async () => {
    for (const child of started) await stop(child)
  })
  await assert.rejects(
    withTestProcesses(async (start) => {
      for (let i = 0; i < 2; i++) {
        const { child } = start(process.execPath, ['-e', 'setTimeout(() => {}, 30000)'], {
          env: {},
        })
        started.push(child)
        await once(child, 'spawn')
      }
      throw new Error('partial startup')
    }),
    /^Error: partial startup$/,
  )
  for (const child of started) assert.ok(child.exitCode !== null || child.signalCode !== null)
})

class TestSocket extends EventTarget {
  send(message) {
    this.lastId = JSON.parse(message).id
  }

  reply(id, result) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ id, result }) }))
  }
}

for (const event of ['close', 'error']) {
  test(`all CDP calls reject on socket ${event}`, { timeout: 2000 }, async () => {
    const ws = new TestSocket()
    const send = cdpRequests(ws)
    const calls = [send('Page.navigate'), send('Runtime.evaluate')]
    ws.dispatchEvent(new Event(event))
    calls.push(send('Page.bringToFront'))
    await Promise.all(calls.map((call) => assert.rejects(call, /disconnected/)))
  })
}

test('a late CDP response cannot complete another request', { timeout: 2000 }, async () => {
  const ws = new TestSocket()
  const send = cdpRequests(ws, 50)
  const first = send('Page.navigate')
  const firstId = ws.lastId
  await assert.rejects(first, /Page.navigate timed out/)
  const second = send('Runtime.evaluate')
  ws.reply(firstId, 'late')
  ws.reply(ws.lastId, 'current')
  assert.equal(await second, 'current')
})
