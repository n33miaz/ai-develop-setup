import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

// O jsdom não tem matchMedia nem canvas, e o xterm.js consulta os dois ao abrir.
const noop = (): void => undefined
const mediaQueryList = {
  matches: false,
  addEventListener: noop,
  removeEventListener: noop,
  addListener: noop,
  removeListener: noop,
}
window.matchMedia = (media: string) => ({ ...mediaQueryList, media }) as unknown as MediaQueryList
HTMLCanvasElement.prototype.getContext = () => null

afterEach(() => {
  cleanup()
  document.documentElement.removeAttribute('data-theme')
  localStorage.clear()
})
