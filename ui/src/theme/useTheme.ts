import { useCallback, useLayoutEffect, useState } from 'react'
import { applyTheme, loadTheme, nextTheme, saveTheme, type ThemePreference } from './theme'

export function useTheme(): { preference: ThemePreference; cycle: () => void } {
  const [preference, setPreference] = useState<ThemePreference>(loadTheme)

  useLayoutEffect(() => {
    applyTheme(preference, document.documentElement)
  }, [preference])

  const cycle = useCallback(() => {
    setPreference((current) => {
      const next = nextTheme(current)
      saveTheme(next)
      return next
    })
  }, [])

  return { preference, cycle }
}
