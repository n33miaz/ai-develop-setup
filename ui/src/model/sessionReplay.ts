import type { SessionEvent } from '../api'

export const SESSION_STREAM_PREFIX = 'session/'

interface Cursor {
  seq: number
  createdId?: string
  finished?: 'exited' | 'closed'
}

/** Guarda um cursor por sessão ativa; só fluxos configurados conservam o cursor após o fim. */
export class SessionReplay {
  private cursors = new Map<string, Cursor>()
  private configured = new Set<string>()

  configure(streams: readonly string[]) {
    this.configured = new Set(
      streams
        .filter((stream) => stream.startsWith(SESSION_STREAM_PREFIX))
        .map((stream) => stream.slice(SESSION_STREAM_PREFIX.length)),
    )
    for (const [id, cursor] of this.cursors) {
      if (cursor.finished && !this.configured.has(id)) this.cursors.delete(id)
    }
  }

  receive(event: SessionEvent): boolean {
    const id = event.data.session_id
    if (event.stream !== SESSION_STREAM_PREFIX + id) return false
    const cursor = this.cursors.get(id)
    if (event.type === 'urn:ads:event:session.created:1') {
      // A nova instância tem outro evento de criação e pode recomeçar a sequência.
      if (cursor?.createdId === event.id) return false
      this.cursors.set(id, { seq: event.seq, createdId: event.id })
      return true
    }
    if (cursor?.finished || (cursor && event.seq <= cursor.seq)) return false
    this.cursors.set(id, { ...cursor, seq: event.seq })
    return true
  }

  creationOf(id: string): string | undefined {
    return this.cursors.get(id)?.createdId
  }

  isFinished(id: string): boolean {
    return this.cursors.get(id)?.finished !== undefined
  }

  finish(id: string, exited = false) {
    if (this.configured.has(id)) {
      this.cursors.set(id, {
        seq: 0,
        ...this.cursors.get(id),
        finished: exited ? 'exited' : 'closed',
      })
    } else {
      this.cursors.delete(id)
    }
  }

  resumeCleanup(id: string) {
    const cursor = this.cursors.get(id)
    if (cursor?.finished === 'exited') delete cursor.finished
  }
}
