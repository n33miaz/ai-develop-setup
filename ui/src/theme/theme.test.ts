import { describe, expect, it } from 'vitest'
import { applyTheme, loadTheme, nextTheme, saveTheme, THEME_STORAGE_KEY } from './theme'

describe('theme preference', () => {
  it('cycles system, light, dark and back', () => {
    expect(nextTheme('system')).toBe('light')
    expect(nextTheme('light')).toBe('dark')
    expect(nextTheme('dark')).toBe('system')
  })

  it('defaults to system when nothing valid is stored', () => {
    expect(loadTheme()).toBe('system')
    localStorage.setItem(THEME_STORAGE_KEY, 'sepia')
    expect(loadTheme()).toBe('system')
  })

  it('reads back the saved preference', () => {
    saveTheme('dark')
    expect(loadTheme()).toBe('dark')
  })

  it('forces a theme through data-theme and removes it for system', () => {
    const root = document.createElement('html')

    applyTheme('dark', root)
    expect(root.getAttribute('data-theme')).toBe('dark')
    applyTheme('light', root)
    expect(root.getAttribute('data-theme')).toBe('light')
    applyTheme('system', root)
    expect(root.hasAttribute('data-theme')).toBe(false)
  })
})
