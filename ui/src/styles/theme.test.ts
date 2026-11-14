import { describe, expect, it } from 'vitest'
import css from './theme.css?raw'

type Tokens = Record<string, string>

function tokensOf(block: string): Tokens {
  const tokens: Tokens = {}
  for (const match of block.matchAll(/(--[\w-]+):\s*(#[0-9a-f]{6})\s*;/gi)) {
    const [, name, value] = match
    if (name && value) tokens[name] = value
  }
  return tokens
}

function block(selector: string): Tokens {
  const start = css.indexOf(`${selector} {`)
  if (start < 0) throw new Error(`missing block ${selector}`)
  return tokensOf(css.slice(start, css.indexOf('}', start)))
}

const light = block(':root')
const darkBySystem = block(":root:not([data-theme='light'])")
const darkByAttribute = block(":root[data-theme='dark']")

function luminance(hex: string): number {
  const channel = (offset: number): number => {
    const value = parseInt(hex.slice(offset, offset + 2), 16) / 255
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
}

function contrast(tokens: Tokens, front: string, back: string): number {
  const a = luminance(tokens[front] ?? '')
  const b = luminance(tokens[back] ?? '')
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
}

// Texto: AA pede 4,5:1. Borda de controle, indicador de foco e ícone: 3:1.
const TEXT_PAIRS: readonly (readonly [string, string])[] = [
  ['--text', '--bg'],
  ['--text', '--surface'],
  ['--text', '--surface-2'],
  ['--text-muted', '--surface'],
  ['--text-muted', '--surface-2'],
  ['--accent-contrast', '--accent'],
  ['--terminal-fg', '--terminal-bg'],
  ['--terminal-red', '--terminal-bg'],
  ['--terminal-green', '--terminal-bg'],
  ['--terminal-yellow', '--terminal-bg'],
  ['--terminal-blue', '--terminal-bg'],
  ['--terminal-cyan', '--terminal-bg'],
  ['--terminal-muted', '--terminal-bg'],
]

const NON_TEXT_PAIRS: readonly (readonly [string, string])[] = [
  ['--border-strong', '--surface'],
  ['--border-strong', '--surface-2'],
  ['--border-strong', '--bg'],
  ['--focus', '--bg'],
  ['--focus', '--surface'],
  ['--focus', '--surface-2'],
  ['--ok', '--surface'],
  ['--warn', '--surface'],
  ['--danger', '--surface'],
]

describe('theme tokens', () => {
  it('defines the same tokens in the light and dark themes', () => {
    expect(Object.keys(light).length).toBeGreaterThan(0)
    expect(Object.keys(darkByAttribute).sort()).toEqual(Object.keys(light).sort())
  })

  it('keeps the two dark blocks identical', () => {
    expect(darkBySystem).toEqual(darkByAttribute)
  })

  describe.each([
    ['light', light],
    ['dark', darkByAttribute],
  ])('%s theme', (_name, tokens) => {
    it.each(TEXT_PAIRS)('%s on %s reaches 4.5:1', (front, back) => {
      expect(contrast(tokens, front, back)).toBeGreaterThanOrEqual(4.5)
    })

    it.each(NON_TEXT_PAIRS)('%s on %s reaches 3:1', (front, back) => {
      expect(contrast(tokens, front, back)).toBeGreaterThanOrEqual(3)
    })
  })
})
