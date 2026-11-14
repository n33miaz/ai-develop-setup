import { render, screen, waitFor } from '@testing-library/react'
import { createRef } from 'react'
import { describe, expect, it } from 'vitest'
import { staticByteSource, type ByteSource } from './byteSource'
import { Terminal, type TerminalHandle } from './Terminal'

function pushSource(): ByteSource & { push(text: string): void; readonly listeners: number } {
  const listeners = new Set<(bytes: Uint8Array) => void>()
  return {
    subscribe(onBytes) {
      listeners.add(onBytes)
      return () => listeners.delete(onBytes)
    },
    push(text) {
      const bytes = new TextEncoder().encode(text)
      for (const listener of listeners) listener(bytes)
    },
    get listeners() {
      return listeners.size
    },
  }
}

describe('Terminal', () => {
  it('writes the bytes delivered by the source, in order', async () => {
    const source = pushSource()
    const ref = createRef<TerminalHandle>()
    render(<Terminal source={source} label="terminal" ref={ref} />)

    source.push('first line\r\n')
    source.push('sec')
    source.push('ond line')

    await waitFor(() => {
      expect(ref.current?.readScreen()).toBe('first line\nsecond line')
    })
  })

  it('interprets escape sequences and multi-byte text instead of printing them raw', async () => {
    const ref = createRef<TerminalHandle>()
    const source = staticByteSource('\x1b[31mnúcleo\x1b[0m ok')
    render(<Terminal source={source} label="t" ref={ref} />)

    await waitFor(() => {
      expect(ref.current?.readScreen()).toBe('núcleo ok')
    })
  })

  it('exposes a labelled region', () => {
    render(<Terminal source={staticByteSource('')} label="Terminal da sessão A" />)

    expect(screen.getByRole('region', { name: 'Terminal da sessão A' })).toBeInTheDocument()
  })

  it('unsubscribes from the source when removed', () => {
    const source = pushSource()
    const { unmount } = render(<Terminal source={source} label="t" />)
    expect(source.listeners).toBe(1)

    unmount()

    expect(source.listeners).toBe(0)
  })

  it('replaces the content when the source changes', async () => {
    const ref = createRef<TerminalHandle>()
    const { rerender } = render(<Terminal source={staticByteSource('one')} label="t" ref={ref} />)
    await waitFor(() => {
      expect(ref.current?.readScreen()).toBe('one')
    })

    rerender(<Terminal source={staticByteSource('two')} label="t" ref={ref} />)

    await waitFor(() => {
      expect(ref.current?.readScreen()).toBe('two')
    })
  })
})
