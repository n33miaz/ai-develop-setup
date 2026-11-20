import { render, waitFor } from '@testing-library/react'
import { createRef } from 'react'
import { describe, expect, it } from 'vitest'
import { createWsCoreClient, encodeBase64 } from '../../api'
import type { CommandRequestV1 } from '../../api/contracts'
import { eventByteSource } from './byteSource'
import { Terminal, type TerminalHandle } from './Terminal'

/** A core that refuses subscriptions past its per-connection limit, like the daemon does. */
class LimitedCore extends EventTarget {
  sent: CommandRequestV1[] = []
  private readonly limit: number
  constructor(limit: number) {
    super()
    this.limit = limit
  }
  send(text: string) {
    const command = JSON.parse(text) as CommandRequestV1
    this.sent.push(command)
    const subscriptions = this.sent.filter((sent) => sent.method === 'events.subscribe').length
    const refused = command.method === 'events.subscribe' && subscriptions > this.limit
    const reply = refused
      ? { jsonrpc: '2.0', id: command.id, error: { code: -32003, message: 'capacity exceeded' } }
      : { jsonrpc: '2.0', id: command.id, result: { accepted: true, replayed: false } }
    queueMicrotask(() => {
      this.receive(reply)
    })
  }
  close() {
    this.dispatchEvent(new Event('close'))
  }
  receive(message: unknown) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(message) }))
  }
}

describe('Terminal on a live connection', () => {
  // 200 mounts of a real terminal take a few seconds under a loaded test run.
  it(
    'survives many remounts on one connection without new core subscriptions',
    { timeout: 30000 },
    async () => {
      const core = new LimitedCore(2)
      const client = createWsCoreClient({
        url: 'ws://127.0.0.1:12345',
        token: 'a'.repeat(43),
        socketFactory: () => core,
      })
      const ready = client.connect()
      core.dispatchEvent(new Event('open'))
      await ready
      const ref = createRef<TerminalHandle>()
      const view = render(
        <Terminal source={eventByteSource(client, 'session/s')} label="t" ref={ref} />,
      )
      const bytes = new TextEncoder().encode('kept output')
      core.receive({
        id: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
        type: 'urn:ads:event:session.output:1',
        ts: '2026-01-01T00:00:00Z',
        stream: 'session/s',
        seq: 1,
        actor: { kind: 'daemon', id: 'core' },
        cause: null,
        data_class: 'private',
        data: {
          session_id: 's',
          offset: 0,
          length: bytes.length,
          payload: encodeBase64(bytes),
          encoding: 'base64',
        },
      })
      // Each new source mounts the terminal again, as switching sessions does.
      for (let round = 0; round < 200; round += 1) {
        view.rerender(
          <Terminal source={eventByteSource(client, 'session/s')} label="t" ref={ref} />,
        )
      }
      await waitFor(() => {
        expect(ref.current?.readScreen()).toBe('kept output')
      })
      expect(core.sent.filter((sent) => sent.method === 'events.subscribe')).toHaveLength(1)
      expect(client.getState()).toBe('connected')
    },
  )
})
