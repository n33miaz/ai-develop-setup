export type ThemePreference = 'system' | 'light' | 'dark'

export const THEME_ORDER: readonly ThemePreference[] = ['system', 'light', 'dark']
export const THEME_STORAGE_KEY = 'ads.theme'

export function nextTheme(current: ThemePreference): ThemePreference {
  const index = THEME_ORDER.indexOf(current)
  return THEME_ORDER[(index + 1) % THEME_ORDER.length] ?? 'system'
}

function isThemePreference(value: string | null): value is ThemePreference {
  return value === 'system' || value === 'light' || value === 'dark'
}

/** Lê a preferência guardada; sem armazenamento (modo privado, bloqueio) volta ao automático. */
export function loadTheme(): ThemePreference {
  try {
    const value = localStorage.getItem(THEME_STORAGE_KEY)
    return isThemePreference(value) ? value : 'system'
  } catch {
    return 'system'
  }
}

export function saveTheme(preference: ThemePreference): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, preference)
  } catch {
    // sem armazenamento: a preferência vale só nesta sessão
  }
}

/** `system` remove o atributo e deixa `prefers-color-scheme` decidir; os outros forçam o tema. */
export function applyTheme(preference: ThemePreference, root: HTMLElement): void {
  if (preference === 'system') root.removeAttribute('data-theme')
  else root.setAttribute('data-theme', preference)
}
