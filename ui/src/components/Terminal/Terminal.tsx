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
  ref?: Ref<TerminalHandle>
}

/** Mostra no xterm.js os bytes que a fonte entrega. Só exibe: não tem conexão nem entrada ainda. */
export function Terminal({ source, label, ref }: TerminalProps) {
  const container = useRef<HTMLDivElement>(null)
  const term = useRef<XTerm | null>(null)

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
      disableStdin: true,
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

    const refit = (): void => {
      try {
        fit.fit()
      } catch {
        // contêiner ainda sem tamanho (oculto ou sem layout)
      }
    }
    refit()
    const resizeObserver =
      typeof ResizeObserver === 'function' ? new ResizeObserver(refit) : undefined
    resizeObserver?.observe(element)
    const stopWatchingTheme = watchTheme(() => {
      xterm.options.theme = readTerminalTheme(document.documentElement)
    })
    const unsubscribe = source.subscribe((bytes) => {
      xterm.write(bytes)
    })

    return () => {
      unsubscribe()
      stopWatchingTheme()
      resizeObserver?.disconnect()
      term.current = null
      xterm.dispose()
    }
  }, [source])

  return (
    <div className={styles.terminal} role="region" aria-label={label}>
      <div ref={container} className={styles.screen} />
    </div>
  )
}
