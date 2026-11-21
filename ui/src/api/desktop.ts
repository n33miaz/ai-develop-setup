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
  { initialMs = 250, maxMs = 5000 } = {},
): () => CoreSocket {
  let lifecycle = Promise.resolve()
  const changeConnection: DesktopBridge['core']['invoke'] = (command, args) => {
    const change = lifecycle.then(() => core.invoke(command, args))
    lifecycle = change.catch(() => undefined)
    return change
  }
  return () => new DesktopSocket(core, changeConnection, initialMs, maxMs)
}

class DesktopSocket extends EventTarget implements CoreSocket {
  private readonly core: DesktopBridge['core']
  private readonly changeConnection: DesktopBridge['core']['invoke']
  private readonly id = crypto.randomUUID()
  private readonly frames: InstanceType<DesktopBridge['core']['Channel']>
  private closed = false
  private outgoing = Promise.resolve()

  private retryTimer: ReturnType<typeof setTimeout> | undefined

  constructor(
    core: DesktopBridge['core'],
    changeConnection: DesktopBridge['core']['invoke'],
    initialMs: number,
    maxMs: number,
  ) {
    super()
    this.core = core
    this.changeConnection = changeConnection
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
    const connect = (delay: number) => {
      if (this.closed) return
      void changeConnection('desktop_connect', { id: this.id, frames: this.frames }).then(
        () => {
          if (!this.closed) this.dispatchEvent(new Event('open'))
        },
        (reason: unknown) => {
          if (this.closed) return
          switch (reason) {
            case 'desktop connection retry':
              this.retryTimer = setTimeout(() => {
                connect(Math.min(delay * 2, maxMs))
              }, delay)
              break
            case 'desktop protocol incompatible':
              this.dispatchEvent(new CustomEvent('error', { detail: 'incompatible' }))
              break
            case 'desktop relaunch required':
              this.dispatchEvent(new CustomEvent('error', { detail: 'relaunch_required' }))
              break
            default:
              this.dispatchEvent(new Event('error'))
          }
        },
      )
    }
    connect(Math.min(initialMs, maxMs))
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
    clearTimeout(this.retryTimer)
    this.frames.onmessage = () => undefined
    void this.changeConnection('desktop_disconnect', { id: this.id }).catch(() => undefined)
  }
}
