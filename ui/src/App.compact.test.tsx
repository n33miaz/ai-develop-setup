import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { expect, it, vi } from 'vitest'
import { createFakeCoreClient } from './api'
import { App } from './App'
import { createTranslator, I18nContext } from './i18n'

it('uses a collapsible navigation at a phone width and gives focus back to the terminal', async () => {
  const phoneWidth = 375
  const matchMedia = window.matchMedia.bind(window)
  const media = vi.spyOn(window, 'matchMedia').mockImplementation((query) =>
    Object.assign(matchMedia(query), {
      matches: query === '(width < 48rem)' && phoneWidth < 48 * 16,
    }),
  )
  try {
    const user = userEvent.setup()
    render(
      <I18nContext value={createTranslator('en')}>
        <App client={createFakeCoreClient()} />
      </I18nContext>,
    )
    const menu = screen.getByRole('button', { name: 'Menu' })
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument()
    expect(screen.getByRole('main')).toBeVisible()
    await user.click(menu)
    expect(menu).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('navigation')).toBeVisible()
    await user.click(screen.getByRole('button', { name: /Sample session B/ }))
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument()
    expect(screen.getByRole('main')).toHaveFocus()
  } finally {
    media.mockRestore()
  }
})

it.each([
  ['pt-BR', 'Núcleo: desconectado', 'Abrir claude', 'Abrir codex'],
  ['en', 'Core: disconnected', 'Open claude', 'Open codex'],
] as const)(
  'names the compact connection and session controls in %s',
  async (locale, status, claude, codex) => {
    const matchMedia = window.matchMedia.bind(window)
    const media = vi
      .spyOn(window, 'matchMedia')
      .mockImplementation((query) =>
        Object.assign(matchMedia(query), { matches: query === '(width < 48rem)' }),
      )
    try {
      const client = createFakeCoreClient()
      const { rerender } = render(
        <I18nContext value={createTranslator(locale)}>
          <App client={client} />
        </I18nContext>,
      )
      expect(screen.getByRole('status', { name: status })).toBeInTheDocument()

      rerender(
        <I18nContext value={createTranslator(locale)}>
          <App client={client} live harnesses={['claude', 'codex']} />
        </I18nContext>,
      )
      expect(await screen.findByRole('button', { name: claude })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: codex })).toBeInTheDocument()
    } finally {
      media.mockRestore()
    }
  },
)
