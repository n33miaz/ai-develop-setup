import { createContext, useContext } from 'react'
import { createTranslator, DEFAULT_LOCALE, type Translate } from './translator'

export const I18nContext = createContext<Translate>(createTranslator(DEFAULT_LOCALE))

export function useT(): Translate {
  return useContext(I18nContext)
}
