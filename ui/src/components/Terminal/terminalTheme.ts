import type { ITheme } from '@xterm/xterm'

const FALLBACK = {
  '--terminal-bg': '#0a0d12',
  '--terminal-fg': '#d7dde5',
  '--terminal-selection': '#264f8f',
  '--terminal-red': '#ff8a82',
  '--terminal-green': '#56d364',
  '--terminal-yellow': '#e3b341',
  '--terminal-blue': '#79b8ff',
  '--terminal-cyan': '#56d4dd',
  '--terminal-muted': '#8b96a3',
} as const

type Token = keyof typeof FALLBACK

/** Monta o tema do xterm a partir das variáveis CSS do tema ativo. */
export function readTerminalTheme(root: HTMLElement): ITheme {
  const style = getComputedStyle(root)
  const read = (token: Token): string => style.getPropertyValue(token).trim() || FALLBACK[token]
  return {
    background: read('--terminal-bg'),
    foreground: read('--terminal-fg'),
    cursor: read('--terminal-fg'),
    selectionBackground: read('--terminal-selection'),
    red: read('--terminal-red'),
    green: read('--terminal-green'),
    yellow: read('--terminal-yellow'),
    blue: read('--terminal-blue'),
    cyan: read('--terminal-cyan'),
    brightBlack: read('--terminal-muted'),
  }
}

/** Avisa quando o tema muda (atributo `data-theme` ou preferência do sistema). */
export function watchTheme(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange)
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  const media =
    typeof window.matchMedia === 'function'
      ? window.matchMedia('(prefers-color-scheme: dark)')
      : undefined
  media?.addEventListener('change', onChange)
  return () => {
    observer.disconnect()
    media?.removeEventListener('change', onChange)
  }
}
