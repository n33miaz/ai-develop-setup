import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createTranslator, I18nContext, type Locale } from '../../i18n'
import type { SessionView, TaskView } from '../../model/views'
import { Sidebar } from './Sidebar'

function renderSidebar(
  tasks: readonly TaskView[],
  sessions: readonly SessionView[],
  locale: Locale = 'pt-BR',
): void {
  render(
    <I18nContext value={createTranslator(locale)}>
      <Sidebar
        id="sidebar"
        open
        tasks={tasks}
        sessions={sessions}
        selectedSessionId={sessions[0]?.id ?? null}
        onSelectSession={() => undefined}
      />
    </I18nContext>,
  )
}

describe('Sidebar', () => {
  it('shows the task title and the session name as they came', () => {
    const title = 'Título do usuário com <b>marcação</b> & símbolos'
    renderSidebar(
      [{ id: 'task-1', title, state: 'pending' }],
      [{ id: 'session-1', displayName: 'nome-do-usuário', activity: 'active' }],
    )

    expect(screen.getByText(title)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^nome-do-usuário\s*Ativa$/ })).toBeInTheDocument()
  })

  it.each([
    ['pt-BR', 'Tarefa sem título', /^Sessão sem nome\s*Ativa$/],
    ['en', 'Untitled task', /^Unnamed session\s*Active$/],
  ] as const)('uses neutral texts for what has no title or name (%s)', (locale, task, session) => {
    renderSidebar(
      [{ id: 'task-1', state: 'pending' }],
      [{ id: 'session-1', activity: 'active' }],
      locale,
    )

    expect(screen.getByText(task)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: session })).toBeInTheDocument()
  })

  it.each([
    ['pt-BR', ['Pendente', 'Em andamento', 'Concluída', 'Falhou', 'Cancelada']],
    ['en', ['Pending', 'In progress', 'Done', 'Failed', 'Cancelled']],
  ] as const)('names every task state in words (%s)', (locale, labels) => {
    const states = ['pending', 'running', 'done', 'failed', 'cancelled'] as const
    renderSidebar(
      states.map((state) => ({ id: `task-${state}`, title: `title-${state}`, state })),
      [],
      locale,
    )

    states.forEach((state, index) => {
      const item = screen.getByText(`title-${state}`).closest('li')
      if (!item) throw new Error(`missing item of ${state}`)
      const label = within(item).getByText(labels[index] ?? '')
      expect(label).toHaveAttribute('data-state', state)
    })
    expect(new Set(labels).size).toBe(states.length)
  })

  it('tells active from idle by text and by the shape of the dot, not only by colour', () => {
    renderSidebar(
      [],
      [
        { id: 'session-1', displayName: 'one', activity: 'active' },
        { id: 'session-2', displayName: 'two', activity: 'idle' },
      ],
    )

    const active = within(screen.getByRole('button', { name: /one/ })).getByText('Ativa')
    const idle = within(screen.getByRole('button', { name: /two/ })).getByText('Ociosa')

    expect(active).toHaveAttribute('data-activity', 'active')
    expect(idle).toHaveAttribute('data-activity', 'idle')
    expect(active.querySelector('[aria-hidden="true"]')).not.toBeNull()
    expect(idle.querySelector('[aria-hidden="true"]')).not.toBeNull()
  })
})
