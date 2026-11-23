import { FitAddon } from '@xterm/addon-fit'
import { Terminal as XTerm } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { useEffect, useImperativeHandle, useRef, type Ref } from 'react'
import type { ByteSource } from './byteSource'
import styles from './Terminal.module.css'
import { readTerminalTheme, watchTheme } from './terminalTheme'

export interface TerminalHandle {
  /** Texto hoje na tela do terminal (linhas sem espaço sobrando no fim). Serve a testes. */
  readScreen(): string
}

interface TerminalProps {
  source: ByteSource
  label: string

  onInput?: ((bytes: Uint8Array) => void) | undefined

  onResize?: ((cols: number, rows: number) => void) | undefined

  resizeDebounceMs?: number
  browserClipboard?: boolean

  ref?: Ref<TerminalHandle>
}

/** Mostra bytes da fonte e encaminha a entrada quando há um consumidor. */

export function Terminal({
  source,
  label,
  ref,
  onInput,
  onResize,
  resizeDebounceMs = 150,
  browserClipboard = true,
}: TerminalProps) {
  const container = useRef<HTMLDivElement>(null)
  const term = useRef<XTerm | null>(null)

  const callbacks = useRef({ onInput, onResize })

  useEffect(() => {
    callbacks.current = { onInput, onResize }

    if (term.current) term.current.options.disableStdin = !onInput
  }, [onInput, onResize])

  useImperativeHandle(ref, () => ({
    readScreen() {
      const buffer = term.current?.buffer.active
      if (!buffer) return ''
      const lines: string[] = []
      for (let row = 0; row < buffer.length; row += 1) {
        lines.push(buffer.getLine(row)?.translateToString(true) ?? '')
      }
      return lines.join('\n').trimEnd()
    },
  }))

  useEffect(() => {
    const element = container.current
    if (!element) return
    const xterm = new XTerm({
      disableStdin: !callbacks.current.onInput,

      cursorBlink: false,
      cursorInactiveStyle: 'none',
      convertEol: false,
      fontFamily: 'ui-monospace, "Cascadia Mono", Consolas, "Liberation Mono", monospace',
      fontSize: 13,
      scrollback: 5000,
      theme: readTerminalTheme(document.documentElement),
    })
    const fit = new FitAddon()
    xterm.loadAddon(fit)
    xterm.open(element)
    term.current = xterm

    if (browserClipboard && !navigator.platform.startsWith('Mac')) {
      xterm.attachCustomKeyEventHandler((event) => {
        if (
          event.type !== 'keydown' ||
          !event.ctrlKey ||
          event.altKey ||
          event.metaKey ||
          event.shiftKey
        )
          return true
        let key = event.key.toLowerCase()
        if (/^\p{L}$/u.test(key) && !/^\p{Script=Latin}$/u.test(key)) {
          // eslint-disable-next-line @typescript-eslint/no-deprecated -- O xterm usa esse código virtual para gerar o byte de Ctrl+letra.
          const virtualKey = event.keyCode
          key =
            virtualKey >= 65 && virtualKey <= 90
              ? String.fromCharCode(virtualKey).toLowerCase()
              : event.code.replace(/^Key/, '').toLowerCase()
        }
        // Deixa o navegador disparar os eventos de clipboard que o xterm já trata.
        return key !== 'v' && (key !== 'c' || !xterm.hasSelection())
      })
    }

    const refit = (): void => {
      try {
        fit.fit()
      } catch {
        // contêiner ainda sem tamanho (oculto ou sem layout)
      }
    }

    let resizeTimer: ReturnType<typeof setTimeout> | undefined

    const resized = xterm.onResize(({ cols, rows }) => {
      clearTimeout(resizeTimer)

      resizeTimer = setTimeout(() => callbacks.current.onResize?.(cols, rows), resizeDebounceMs)
    })

    let historical = false

    const input = xterm.onData((data) => {
      if (!historical) callbacks.current.onInput?.(new TextEncoder().encode(data))
    })

    const binary = xterm.onBinary((data) => {
      if (!historical)
        callbacks.current.onInput?.(Uint8Array.from(data, (char) => char.charCodeAt(0)))
    })

    refit()
    const resizeObserver =
      typeof ResizeObserver === 'function' ? new ResizeObserver(refit) : undefined
    resizeObserver?.observe(element)
    const stopWatchingTheme = watchTheme(() => {
      xterm.options.theme = readTerminalTheme(document.documentElement)
    })

    // Serializa a análise assíncrona: cada resposta pertence ao bloco em análise.

    const queue: { bytes: Uint8Array; replay: boolean }[] = []

    let writing = false

    let disposed = false

    const drain = () => {
      if (writing || disposed) return

      const chunk = queue.shift()

      if (!chunk) return

      writing = true

      historical = chunk.replay

      xterm.write(chunk.bytes, () => {
        historical = false

        writing = false

        drain()
      })
    }

    const unsubscribe = source.subscribe((bytes, replay = false) => {
      queue.push({ bytes, replay })

      drain()
    })

    if (callbacks.current.onInput) xterm.focus()

    return () => {
      disposed = true

      clearTimeout(resizeTimer)

      resized.dispose()

      input.dispose()

      binary.dispose()

      unsubscribe()
      stopWatchingTheme()
      resizeObserver?.disconnect()
      term.current = null
      xterm.dispose()
    }
  }, [source, resizeDebounceMs, browserClipboard])

  return (
    <div className={styles.terminal} role="region" aria-label={label}>
      <div ref={container} className={styles.screen} />
    </div>
  )
}
