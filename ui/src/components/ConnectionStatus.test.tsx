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
      'proof_missing',
      'pt-BR',
      'Núcleo: o serviço na porta local não comprovou sua identidade. Reabra pelo lançador.',
    ],
    ['disconnected', 'en', 'Core: disconnected'],
    ['relaunch_required', 'en', 'Core: connection unavailable. Reopen from the launcher.'],
    [
      'proof_missing',
      'en',
      'Core: the service on the local port did not prove its identity. Reopen from the launcher.',
    ],
    [
      'retry_exhausted',
      'pt-BR',
      'Núcleo: limite de tentativas de conexão atingido. Reabra pelo lançador.',
    ],
    ['retry_exhausted', 'en', 'Core: connection attempt limit reached. Reopen from the launcher.'],
  ])('labels %s in %s', (state, locale, label) => {
    render(
      <I18nContext.Provider value={createTranslator(locale)}>
        <ConnectionStatus state={state} />
      </I18nContext.Provider>,
    )
    expect(screen.getByRole('status', { name: label })).toBeInTheDocument()
  })
})
