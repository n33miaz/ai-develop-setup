import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ConnectionState } from '../api'
import { createTranslator, I18nContext, type Locale } from '../i18n'
import { ConnectionStatus } from './ConnectionStatus'

describe('ConnectionStatus', () => {
  it.each<[ConnectionState, Locale, string]>([
    ['disconnected', 'pt-BR', 'Núcleo: desconectado'],
    [
      'relaunch_required',
      'pt-BR',
      'Núcleo: conexão indisponível. Reinicie o núcleo pelo sinal restart do lançador ou pare e inicie o lançador.',
    ],
    [
      'proof_missing',
      'pt-BR',
      'Núcleo: o serviço na porta local não comprovou sua identidade. Confira se a janela e o núcleo têm a mesma versão. Reinicie o núcleo pelo sinal restart do lançador ou pare e inicie o lançador.',
    ],
    ['disconnected', 'en', 'Core: disconnected'],
    [
      'relaunch_required',
      'en',
      'Core: connection unavailable. Restart the core with the launcher restart signal, or stop and start the launcher.',
    ],
    [
      'proof_missing',
      'en',
      'Core: the service on the local port did not prove its identity. Check that the window and the core have the same version. Restart the core with the launcher restart signal, or stop and start the launcher.',
    ],
    [
      'retry_exhausted',
      'pt-BR',
      'Núcleo: limite de tentativas de conexão atingido. Reinicie o núcleo pelo sinal restart do lançador ou pare e inicie o lançador.',
    ],
    [
      'retry_exhausted',
      'en',
      'Core: connection attempt limit reached. Restart the core with the launcher restart signal, or stop and start the launcher.',
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
