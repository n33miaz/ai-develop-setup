import { en, ptBR, type MessageKey } from './messages'

export type Locale = 'pt-BR' | 'en'
export const DEFAULT_LOCALE: Locale = 'pt-BR'

export type Translate = (key: MessageKey, vars?: Record<string, string>) => string

const catalogs: Record<Locale, Record<MessageKey, string>> = { 'pt-BR': ptBR, en }

/** Qualquer coisa que não seja inglês cai no padrão (pt-BR). */
export function resolveLocale(value: string | null | undefined): Locale {
  return value?.toLowerCase().startsWith('en') ? 'en' : DEFAULT_LOCALE
}

export function createTranslator(locale: Locale): Translate {
  const catalog = catalogs[locale]
  return (key, vars) => {
    const text = catalog[key]
    if (!vars) return text
    return text.replace(/\{(\w+)\}/g, (whole, name: string) => vars[name] ?? whole)
  }
}
