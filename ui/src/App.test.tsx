import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { createFakeCoreClient, type FakeCoreClient } from './api'
import { App } from './App'
import { createTranslator, I18nContext, type Locale } from './i18n'
import { THEME_STORAGE_KEY } from './theme/theme'

function renderApp(locale: Locale = 'pt-BR'): FakeCoreClient {
  const client = createFakeCoreClient()
  render(
    <I18nContext value={createTranslator(locale)}>
      <App client={client} />
    </I18nContext>,
  )
  return client
}

describe('App', () => {
  it('shows the final structure: sidebar, terminal area and core connection', () => {
    renderApp()

    const nav = screen.getByRole('navigation', { name: 'Tarefas e sessões' })
    expect(within(nav).getByRole('heading', { name: 'Caixa de tarefas' })).toBeInTheDocument()
    expect(within(nav).getByRole('heading', { name: 'Sessões' })).toBeInTheDocument()
    expect(within(nav).getAllByRole('listitem')).toHaveLength(6)
    expect(
      screen.getByRole('region', { name: 'Terminal da sessão Sessão de exemplo A' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Núcleo: desconectado')
    expect(screen.getByText(/Em construção/)).toBeInTheDocument()
  })

  it('follows the connection state of the client', async () => {
    const client = renderApp()

    await act(() => client.connect())
    expect(screen.getByRole('status')).toHaveTextContent('Núcleo: conectado')
    expect(screen.getByRole('status')).toHaveAttribute('data-state', 'connected')

    act(() => {
      client.disconnect()
    })
    expect(screen.getByRole('status')).toHaveTextContent('Núcleo: desconectado')
  })

  it('selects a session and points the terminal at it', async () => {
    const user = userEvent.setup()
    renderApp()

    await user.click(screen.getByRole('button', { name: /Sessão de exemplo B/ }))

    expect(screen.getByRole('button', { name: /Sessão de exemplo B/ })).toHaveAttribute(
      'aria-current',
      'true',
    )
    expect(screen.getByRole('button', { name: /Sessão de exemplo A/ })).toHaveAttribute(
      'aria-current',
      'false',
    )
    expect(
      screen.getByRole('region', { name: 'Terminal da sessão Sessão de exemplo B' }),
    ).toBeInTheDocument()
  })

  it('opens and closes the sidebar from the menu button', async () => {
    const user = userEvent.setup()
    renderApp()
    const menu = screen.getByRole('button', { name: 'Menu' })
    const nav = screen.getByRole('navigation')
    expect(menu).toHaveAttribute('aria-expanded', 'false')
    expect(menu).toHaveAttribute('aria-controls', nav.id)
    expect(nav).toHaveAttribute('data-open', 'false')

    await user.click(menu)
    expect(menu).toHaveAttribute('aria-expanded', 'true')
    expect(nav).toHaveAttribute('data-open', 'true')

    await user.click(screen.getByRole('button', { name: /Sessão de exemplo C/ }))
    expect(menu).toHaveAttribute('aria-expanded', 'false')
  })

  it('cycles the theme, sets data-theme and remembers the choice', async () => {
    const user = userEvent.setup()
    renderApp()
    const root = document.documentElement
    expect(root.hasAttribute('data-theme')).toBe(false)

    await user.click(screen.getByRole('button', { name: 'Tema: automático' }))
    expect(root.getAttribute('data-theme')).toBe('light')

    await user.click(screen.getByRole('button', { name: 'Tema: claro' }))
    expect(root.getAttribute('data-theme')).toBe('dark')
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('dark')

    await user.click(screen.getByRole('button', { name: 'Tema: escuro' }))
    expect(root.hasAttribute('data-theme')).toBe(false)
  })

  it('starts with the stored theme', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'dark')

    renderApp()

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  it('reaches every control by keyboard, starting at the skip link', async () => {
    const user = userEvent.setup()
    renderApp()

    await user.tab()
    expect(screen.getByRole('link', { name: 'Pular para o terminal' })).toHaveFocus()
    expect(screen.getByRole('link')).toHaveAttribute('href', '#terminal')
    await user.tab()
    expect(screen.getByRole('button', { name: 'Menu' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: /Tema/ })).toHaveFocus()
  })

  it('renders in english', () => {
    renderApp('en')

    expect(screen.getByRole('navigation', { name: 'Tasks and sessions' })).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Core: disconnected')
    expect(screen.getByRole('button', { name: 'Theme: automatic' })).toBeInTheDocument()
  })
})
