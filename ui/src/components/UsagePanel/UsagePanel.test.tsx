import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { createTranslator, I18nContext } from '../../i18n'
import { sampleUsage } from '../../sample/usage'
import { UsagePanel } from './UsagePanel'

describe('UsagePanel', () => {
  it.each(['{Enter}', ' '])('opens with %j and tabs past the disabled picker', async (key) => {
    const user = userEvent.setup()
    render(
      <>
        <UsagePanel tools={sampleUsage} />
        <button type="button">Continuar</button>
      </>,
    )
    await user.tab()
    const summary = screen.getByText('Uso e contas')
    expect(summary).toHaveFocus()
    const details = summary.closest('details')
    expect(details).not.toHaveAttribute('open')
    await user.keyboard(key)
    expect(details).toHaveAttribute('open')
    const panel = screen.getByRole('region', { name: 'Uso e contas' })
    await user.tab()
    expect(panel).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Continuar' })).toHaveFocus()

    const tool = within(panel).getByRole('region', { name: 'harness-a' })
    const picker = within(tool).getByRole('combobox', { name: 'Perfil da ferramenta harness-a' })
    expect(picker).toBeDisabled()
    expect(picker).toHaveAccessibleDescription(
      'A troca de conta fica desativada até a habilitação dos perfis.',
    )
    expect(picker).toHaveDisplayValue('secondary (próxima sessão)')
    expect(within(picker).getByRole('option', { name: 'primary' })).toBeInTheDocument()
    expect(within(tool).getByText('4% de uso observado · 96% de cota restante')).toBeVisible()
    expect(within(tool).getAllByText('Sem observação nesta janela.')).toHaveLength(3)
    expect(within(tool).getByText(/Orçamento restante da tarefa task-01.*2,60/)).toBeVisible()
    expect(within(tool).getByText(/somente observação.*primary, provider-a, model-x/)).toBeVisible()
  })

  it('reports the local API limitation in English without rendering example profiles', async () => {
    const user = userEvent.setup()
    render(
      <I18nContext value={createTranslator('en')}>
        <UsagePanel tools={null} />
      </I18nContext>,
    )
    await user.tab()
    await user.keyboard('{Enter}')
    expect(screen.getByText(/not yet available from the local API/)).toBeVisible()
    expect(screen.queryByRole('combobox')).toBeNull()
    expect(screen.queryByText('harness-a')).toBeNull()
  })
})
