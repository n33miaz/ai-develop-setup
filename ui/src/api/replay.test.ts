import { describe, expect, it } from 'vitest'
import type { EventEnvelopeV1 } from './contracts'
import {
  covers,
  DEFAULT_REPLAY_BYTES,
  DEFAULT_REPLAY_EVENTS,
  remember,
  replayLimit,
  type ReplayBuffer,
} from './replay'

function event(seq: number): EventEnvelopeV1 {
  return {
    id: '01ARZ3NDEKTSV4RRFFQ69G5FAB',
    type: 'urn:ads:event:session.resized:1',
    ts: '2030-01-02T03:04:05Z',
    stream: 'session/sess-01',
    seq,
    actor: { kind: 'daemon', id: 'core-01' },
    cause: null,
    data_class: 'internal',
    data: { session_id: 'sess-01', cols: 100, rows: 30 },
  }
}

function emptyBuffer(start = 0, last = start): ReplayBuffer {
  return { start, last, events: [], bytes: 0 }
}

describe('replayLimit', () => {
  it.each([undefined, 0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])(
    'falls back to the defaults for %s',
    (value) => {
      expect(replayLimit(value, DEFAULT_REPLAY_EVENTS)).toBe(DEFAULT_REPLAY_EVENTS)
      expect(replayLimit(value, DEFAULT_REPLAY_BYTES)).toBe(DEFAULT_REPLAY_BYTES)
    },
  )

  it('keeps positive safe integers', () => {
    expect(replayLimit(1, DEFAULT_REPLAY_EVENTS)).toBe(1)
    expect(replayLimit(Number.MAX_SAFE_INTEGER, DEFAULT_REPLAY_BYTES)).toBe(Number.MAX_SAFE_INTEGER)
  })
})

describe('remember', () => {
  it('evicts the oldest event when the event limit is exceeded', () => {
    const buffer = emptyBuffer()
    const second = event(2)
    const third = event(3)

    remember(buffer, event(1), 3, 2, 100)
    remember(buffer, second, 5, 2, 100)
    remember(buffer, third, 7, 2, 100)

    expect(buffer.events).toEqual([
      { event: second, size: 5 },
      { event: third, size: 7 },
    ])
    expect(buffer.bytes).toBe(12)
  })

  it('evicts as many oldest events as needed when the byte limit is exceeded', () => {
    const buffer = emptyBuffer()
    const third = event(3)

    remember(buffer, event(1), 2, 10, 5)
    remember(buffer, event(2), 2, 10, 5)
    remember(buffer, third, 5, 10, 5)

    expect(buffer.events).toEqual([{ event: third, size: 5 }])
    expect(buffer.bytes).toBe(5)
  })

  it.each([3, 2])('ignores sequence %i after retaining sequence 3', (seq) => {
    const buffer = emptyBuffer()
    const third = event(3)

    remember(buffer, third, 5, 2, 10)
    remember(buffer, event(seq), 7, 2, 10)

    expect(buffer.events).toEqual([{ event: third, size: 5 }])
    expect(buffer.bytes).toBe(5)
  })
})

describe('covers', () => {
  it('covers a cursor at or ahead of the last sequence without retained events', () => {
    const buffer = emptyBuffer(2, 5)

    expect(covers(buffer, 5)).toBe(true)
    expect(covers(buffer, 6)).toBe(true)
  })

  it('rejects a cursor before the requested start even if the next event is retained', () => {
    const buffer = emptyBuffer(3, 5)
    remember(buffer, event(3), 1, 10, 10)

    expect(covers(buffer, 2)).toBe(false)
  })

  it('covers a zero cursor when the requested start is zero, even with an empty buffer', () => {
    expect(covers(emptyBuffer(0, 5), 0)).toBe(true)
  })

  it('rejects a zero cursor when the requested start is positive', () => {
    expect(covers(emptyBuffer(2, 5), 0)).toBe(false)
  })

  it('rejects a cursor that needs events when the buffer is empty', () => {
    expect(covers(emptyBuffer(2, 5), 2)).toBe(false)
  })

  it('covers a cursor whose next event is the oldest retained event or later', () => {
    const buffer = emptyBuffer(1, 5)
    remember(buffer, event(4), 1, 10, 10)
    remember(buffer, event(5), 1, 10, 10)

    expect(covers(buffer, 3)).toBe(true)
    expect(covers(buffer, 4)).toBe(true)
  })

  it('rejects a cursor whose next event is older than the oldest retained event', () => {
    const buffer = emptyBuffer(1, 5)
    remember(buffer, event(4), 1, 10, 10)
    remember(buffer, event(5), 1, 10, 10)

    expect(covers(buffer, 2)).toBe(false)
  })
})
