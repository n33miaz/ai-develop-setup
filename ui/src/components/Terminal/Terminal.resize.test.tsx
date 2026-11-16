import { act, render, waitFor } from '@testing-library/react'
import type { Terminal as XTerm } from '@xterm/xterm'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Terminal } from './Terminal'
import { staticByteSource } from './byteSource'

const layout = vi.hoisted(() => ({ cols: 100, rows: 30, refit: (): void => undefined }))
vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class {
    terminal?: XTerm
    activate(terminal: XTerm) {
      this.terminal = terminal
    }
    fit() {
      this.terminal?.resize(layout.cols, layout.rows)
    }
    dispose() {
      return undefined
    }
  },
}))
afterEach(() => vi.unstubAllGlobals())

describe('terminal sizing', () => {
  it('debounces fitted dimensions and cancels pending notifications on unmount', async () => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(callback: () => void) {
          layout.refit = callback
        }
        observe() {
          return undefined
        }
        disconnect() {
          return undefined
        }
      },
    )
    const resized = vi.fn()
    const { unmount } = render(
      <Terminal source={staticByteSource('')} label="t" onResize={resized} resizeDebounceMs={30} />,
    )
    act(() => {
      layout.cols = 110
      layout.refit()
      layout.cols = 120
      layout.rows = 40
      layout.refit()
    })
    expect(resized).not.toHaveBeenCalled()
    await waitFor(() => {
      expect(resized).toHaveBeenCalledExactlyOnceWith(120, 40)
    })
    act(() => {
      layout.cols = 130
      layout.refit()
    })
    unmount()
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(resized).toHaveBeenCalledOnce()
  })
})
