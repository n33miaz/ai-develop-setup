import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createRef } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { eventByteSource, staticByteSource, type ByteSource } from './byteSource'
import { createFakeCoreClient, encodeBase64 } from '../../api'
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
  it('renders replay and live session output bytes', async () => {
    const client = createFakeCoreClient()
    await client.connect()
    const output = (text: string, offset: number) => {
      const bytes = new TextEncoder().encode(text)
      client.emit({
        type: 'urn:ads:event:session.output:1',
        stream: 'session/session-a',
        data: {
          session_id: 'session-a',
          offset,
          length: bytes.length,
          payload: encodeBase64(bytes),
        },
      })
    }
    output('olá\r\n', 0)
    const ref = createRef<TerminalHandle>()
    render(
      <Terminal source={eventByteSource(client, 'session/session-a')} label="terminal" ref={ref} />,
    )
    output('live output', 6)
    await waitFor(() => {
      expect(ref.current?.readScreen()).toBe('olá\nlive output')
    })
  })
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

describe('terminal input and replay', () => {
  it.each([true, false])('leaves Ctrl+V to the browser when enabled (%s)', (browserClipboard) => {
    const input = vi.fn<(bytes: Uint8Array) => void>()
    const { container } = render(
      <Terminal
        source={staticByteSource('')}
        label="t"
        onInput={input}
        browserClipboard={browserClipboard}
      />,
    )
    const textarea = container.querySelector('textarea')
    if (!textarea) throw new Error('missing terminal input')
    const allowsDefault = fireEvent.keyDown(textarea, { key: 'v', keyCode: 86, ctrlKey: true })
    expect(allowsDefault).toBe(browserClipboard)
    expect(input.mock.calls.map(([bytes]) => [...bytes])).toEqual(browserClipboard ? [] : [[0x16]])
  })

  it('keeps the terminal read only without an input callback', () => {
    const { container } = render(<Terminal source={staticByteSource('')} label="t" />)
    expect(container.querySelector('textarea')).toHaveAttribute('readonly')
  })
  it('sends pasted unicode and keyboard control bytes', () => {
    const input = vi.fn<(bytes: Uint8Array) => void>()
    const { container } = render(
      <Terminal source={staticByteSource('')} label="t" onInput={input} />,
    )
    const textarea = container.querySelector('textarea')
    if (!textarea) throw new Error('missing terminal input')
    expect(textarea).not.toHaveAttribute('readonly')
    fireEvent.paste(textarea, { clipboardData: { getData: () => 'olá😀' } })
    fireEvent.keyDown(textarea, { key: 'c', keyCode: 67, ctrlKey: true })
    expect(input.mock.calls.map(([bytes]) => [...bytes])).toEqual([
      [...new TextEncoder().encode('olá😀')],
      [3],
    ])
  })
  it('drops historical query replies and permits live queries and typing afterwards', async () => {
    let push: (bytes: Uint8Array, replay?: boolean) => void = () => undefined
    const source: ByteSource = {
      subscribe(callback) {
        push = callback
        return () => undefined
      },
    }
    const input = vi.fn<(bytes: Uint8Array) => void>()
    const ref = createRef<TerminalHandle>()
    const { container } = render(<Terminal ref={ref} source={source} label="t" onInput={input} />)
    act(() => {
      push(new TextEncoder().encode('history\x1b[6n\x1b[c'), true)
    })
    await waitFor(() => {
      expect(ref.current?.readScreen()).toBe('history')
    })
    expect(input).not.toHaveBeenCalled()
    act(() => {
      push(new TextEncoder().encode('live\x1b[6n'), false)
    })
    await waitFor(() => {
      expect(input).toHaveBeenCalledOnce()
    })
    const textarea = container.querySelector('textarea')
    if (!textarea) throw new Error('missing terminal input')
    fireEvent.paste(textarea, {
      clipboardData: { getData: () => 'hello' },
    })
    expect(new TextDecoder().decode(input.mock.calls.at(-1)?.[0])).toBe('hello')
  })
})
