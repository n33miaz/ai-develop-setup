import { describe, expect, it } from 'vitest'
import { en, ptBR, type MessageKey } from './messages'
import { createTranslator, resolveLocale } from './translator'

describe('i18n', () => {
  it('has the same keys and placeholders in both catalogs', () => {
    const placeholders = (text: string) => [...text.matchAll(/\{\w+\}/g)].map(String).sort()

    expect(Object.keys(en).sort()).toEqual(Object.keys(ptBR).sort())
    for (const key of Object.keys(ptBR) as MessageKey[]) {
      expect(en[key], key).not.toBe('')
      expect(placeholders(en[key]), key).toEqual(placeholders(ptBR[key]))
    }
  })

  it('fills in variables and keeps unknown placeholders', () => {
    const t = createTranslator('en')

    expect(t('theme.button', { mode: 'dark' })).toBe('Theme: dark')
    expect(t('theme.button', {})).toBe('Theme: {mode}')
  })

  it('translates by locale', () => {
    expect(createTranslator('en')('tasks.heading')).toBe('Task inbox')
    expect(createTranslator('pt-BR')('tasks.heading')).toBe('Caixa de tarefas')
  })

  it('falls back to pt-BR for anything that is not english', () => {
    expect(resolveLocale('en-US')).toBe('en')
    expect(resolveLocale('EN')).toBe('en')
    expect(resolveLocale('es-AR')).toBe('pt-BR')
    expect(resolveLocale(undefined)).toBe('pt-BR')
  })
})
