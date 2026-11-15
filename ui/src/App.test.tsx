import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { createFakeCoreClient, type FakeCoreClient } from './api'
import { App } from './App'
import { createTranslator, I18nContext, type Locale } from './i18n'
import type { SampleEvents } from './sample/data'
import { THEME_STORAGE_KEY } from './theme/theme'

function renderApp(locale: Locale = 'pt-BR', events?: () => SampleEvents): FakeCoreClient {
  const client = createFakeCoreClient()
  render(
    <I18nContext value={createTranslator(locale)}>
      {events ? <App client={client} events={events} /> : <App client={client} />}
    </I18nContext>,
  )
  return client
}

// Eventos sem `title` nem `display_name`, que são opcionais nos contratos.
const UNNAMED: SampleEvents = {
  tasks: [
    {
      type: 'urn:ads:event:task.created:1',
      stream: 'task/task-9',
      data: {
        task_id: 'task-9',
        parent_task_id: null,
        project_id: 'sample-project',
        objective_hash: `sha256:${'e'.repeat(64)}`,
        kind: 'sample',
        risk: 'low',
        oracle_declared: false,
      },
    },
  ],
  sessions: [
    {
      type: 'urn:ads:event:session.created:1',
      stream: 'session/session-9',
      data: {
        session_id: 'session-9',
        task_id: null,
        harness: 'sample-harness',
        mode: 'interactive',
        cols: 80,
        rows: 24,
        workdir_alias: 'sample-workdir',
      },
    },
  ],
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

  it('shows the title, the name and the activity carried by the events', () => {
    renderApp()

    const nav = screen.getByRole('navigation')
    expect(within(nav).getByText('Revisar o plano do módulo de contratos')).toBeInTheDocument()
    expect(within(nav).queryByText('Tarefa sem título')).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /^Sessão de exemplo A\s*Ativa$/ }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /^Sessão de exemplo B\s*Ociosa$/ }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /^Sessão de exemplo C\s*Ociosa$/ }),
    ).toBeInTheDocument()
  })

  it('falls back to neutral texts when the events bring no title and no name', () => {
    renderApp('pt-BR', () => UNNAMED)

    const nav = screen.getByRole('navigation')
    expect(within(nav).getByText('Tarefa sem título')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^Sessão sem nome\s*Ativa$/ })).toBeInTheDocument()
    expect(
      screen.getByRole('region', { name: 'Terminal da sessão Sessão sem nome' }),
    ).toBeInTheDocument()
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
    expect(screen.getByRole('button', { name: /^Sample session B\s*Idle$/ })).toBeInTheDocument()
  })
})
