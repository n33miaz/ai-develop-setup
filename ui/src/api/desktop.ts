import type { CoreSocket } from './ws'

type Frame = { kind: 'message'; text: string } | { kind: 'closed' }

export interface DesktopBridge {
  core: {
    invoke: (command: string, args: Record<string, unknown>) => Promise<void>
    Channel: new () => { onmessage: (frame: Frame) => void }
  }
}

export function desktopSocketFactory(
  core: DesktopBridge['core'],
  { initialMs = 250, maxMs = 5000, stableMs = 10000, maxAttempts = 12 } = {},
): (reconnecting: boolean) => CoreSocket {
  let lifecycle = Promise.resolve()
  const changeConnection: DesktopBridge['core']['invoke'] = (command, args) => {
    const change = lifecycle.then(() => core.invoke(command, args))
    lifecycle = change.catch(() => undefined)
    return change
  }
  const initialDelay = Math.min(initialMs, maxMs)
  let delay = initialDelay
  let attempts = 0
  const reset = () => {
    delay = initialDelay
    attempts = 0
  }
  const nextDelay = () => {
    if (attempts >= maxAttempts) return undefined
    const next = delay
    delay = Math.min(delay * 2, maxMs)
    attempts += 1
    return next
  }
  return (reconnecting) => {
    if (!reconnecting) reset()
    return new DesktopSocket(core, changeConnection, reconnecting, nextDelay, (uptime) => {
      if (uptime >= stableMs) reset()
    })
  }
}

class DesktopSocket extends EventTarget implements CoreSocket {
  private readonly core: DesktopBridge['core']
  private readonly changeConnection: DesktopBridge['core']['invoke']
  private readonly recordUptime: (uptime: number) => void
  private readonly id = crypto.randomUUID()
  private readonly frames: InstanceType<DesktopBridge['core']['Channel']>
  private closed = false
  private outgoing = Promise.resolve()
  private openedAt: number | undefined

  private retryTimer: ReturnType<typeof setTimeout> | undefined

  constructor(
    core: DesktopBridge['core'],
    changeConnection: DesktopBridge['core']['invoke'],
    reconnecting: boolean,
    nextDelay: () => number | undefined,
    recordUptime: (uptime: number) => void,
  ) {
    super()
    this.core = core
    this.changeConnection = changeConnection
    this.recordUptime = recordUptime
    this.frames = new core.Channel()
    this.frames.onmessage = (frame) => {
      if (this.closed) return
      if (frame.kind === 'closed') {
        this.close()
        this.dispatchEvent(new Event('close'))
      } else {
        this.dispatchEvent(new MessageEvent('message', { data: frame.text }))
      }
    }
    const retry = () => {
      const delay = nextDelay()
      if (delay === undefined) {
        queueMicrotask(() => {
          if (!this.closed) {
            this.dispatchEvent(new CustomEvent('error', { detail: 'retry_exhausted' }))
          }
        })
        return
      }
      this.retryTimer = setTimeout(connect, delay)
    }
    const connect = () => {
      if (this.closed) return
      void changeConnection('desktop_connect', { id: this.id, frames: this.frames }).then(
        () => {
          if (this.closed) return
          this.openedAt = performance.now()
          this.dispatchEvent(new Event('open'))
        },
        (reason: unknown) => {
          if (this.closed) return
          switch (reason) {
            case 'desktop connection retry':
              retry()
              break
            case 'desktop identity proof missing':
              this.dispatchEvent(new CustomEvent('error', { detail: 'proof_missing' }))
              break
            case 'desktop relaunch required':
              this.dispatchEvent(new CustomEvent('error', { detail: 'relaunch_required' }))
              break
            default:
              if (reconnecting) retry()
              else this.dispatchEvent(new Event('error'))
          }
        },
      )
    }
    if (reconnecting) retry()
    else connect()
  }

  send(text: string) {
    this.outgoing = this.outgoing
      .then(() =>
        this.closed ? undefined : this.core.invoke('desktop_send', { id: this.id, text }),
      )
      .catch(() => {
        this.dispatchEvent(new Event('error'))
      })
  }

  close() {
    if (this.closed) return
    this.closed = true
    if (this.openedAt !== undefined) this.recordUptime(performance.now() - this.openedAt)
    clearTimeout(this.retryTimer)
    this.frames.onmessage = () => undefined
    void this.changeConnection('desktop_disconnect', { id: this.id }).catch(() => undefined)
  }
}
