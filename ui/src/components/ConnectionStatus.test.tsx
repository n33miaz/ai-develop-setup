import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ConnectionState } from '../api'
import { createTranslator, I18nContext, type Locale } from '../i18n'
import { ConnectionStatus } from './ConnectionStatus'

describe('ConnectionStatus', () => {
  it.each<[ConnectionState, Locale, string]>([
    ['disconnected', 'pt-BR', 'Núcleo: desconectado'],
    ['relaunch_required', 'pt-BR', 'Núcleo: conexão indisponível. Reabra pelo lançador.'],
    [
      'incompatible',
      'pt-BR',
      'Núcleo: prova de identidade ausente. Confira se a casca e o daemon têm a mesma versão.',
    ],
    ['disconnected', 'en', 'Core: disconnected'],
    ['relaunch_required', 'en', 'Core: connection unavailable. Reopen from the launcher.'],
    [
      'incompatible',
      'en',
      'Core: identity proof missing. Check that the shell and daemon are the same version.',
    ],
  ])('labels %s in %s', (state, locale, label) => {
    render(
      <I18nContext.Provider value={createTranslator(locale)}>
        <ConnectionStatus state={state} />
      </I18nContext.Provider>,
    )
    expect(screen.getByRole('status', { name: label })).toBeInTheDocument()
  })
})
