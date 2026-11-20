import { expect, it } from 'vitest'
import { createFakeCoreClient, type SessionEvent } from '../api'
import { SessionReplay } from './sessionReplay'

function fixture() {
  const client = createFakeCoreClient()
  const stream = 'session/session-a'
  const created = () =>
    client.emit({
      type: 'urn:ads:event:session.created:1',
      stream,
      data: { session_id: 'session-a', display_name: 'Session A' },
    }) as SessionEvent
  const activity = (value: 'active' | 'idle') =>
    client.emit({
      type: 'urn:ads:event:session.activity_changed:1',
      stream,
      data: { session_id: 'session-a', activity: value },
    }) as SessionEvent
  return { stream, created, activity }
}

it('ignores replay behind the latest sequence without retaining each event', () => {
  const replay = new SessionReplay()
  const { created, activity } = fixture()
  const first = created()
  const idle = activity('idle')
  const active = activity('active')
  expect(replay.receive(first)).toBe(true)
  expect(replay.receive(active)).toBe(true)
  expect(replay.receive(idle)).toBe(false)
  expect(replay.receive(first)).toBe(false)
})

it('forgets a finished session once its stream is no longer configured', () => {
  const replay = new SessionReplay()
  const { stream, created } = fixture()
  replay.configure([stream])
  const first = created()
  expect(replay.receive(first)).toBe(true)
  replay.finish('session-a')
  expect(replay.isFinished('session-a')).toBe(true)
  expect(replay.receive(first)).toBe(false)
  replay.configure([])
  expect(replay.isFinished('session-a')).toBe(false)
})

it('drops finished sessions opened outside the configured streams', () => {
  const replay = new SessionReplay()
  const { created } = fixture()
  replay.receive(created())
  replay.finish('session-a')
  expect(replay.isFinished('session-a')).toBe(false)
  expect(replay.receive({ ...created(), seq: 1 })).toBe(true)
})
