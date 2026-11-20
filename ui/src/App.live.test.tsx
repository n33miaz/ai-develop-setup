import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { createFakeCoreClient } from './api'
import { isLocalCommand } from './api/wire'
import { App } from './App'
import { createTranslator, I18nContext } from './i18n'
import { sampleSessionEvents } from './sample/data'

describe('live interface', () => {
  it('reconstructs sessions from subscribed events without sample content', async () => {
    const client = createFakeCoreClient()
    const t = createTranslator('en')
    const { unmount } = render(
      <StrictMode>
        <I18nContext value={t}>
          <App client={client} live sessionStreams={['session/session-a']} />
        </I18nContext>
      </StrictMode>,
    )
    await waitFor(() => {
      expect(client.getState()).toBe('connected')
    })
    expect(screen.getByText('No sessions.')).toBeInTheDocument()
    expect(screen.queryByText(t('notice.wip'))).not.toBeInTheDocument()
    act(() => {
      for (const draft of sampleSessionEvents(t)) {
        if (draft.stream === 'session/session-a') client.emit(draft)
      }
    })
    expect(
      screen.getByRole('region', { name: t('terminal.label', { name: t('sample.session.a') }) }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Sample session B/ })).not.toBeInTheDocument()
    act(() => {
      client.emit({
        type: 'urn:ads:event:session.ended:1',
        stream: 'session/session-a',
        data: { session_id: 'session-a', reason: 'exit', exit_code: 0 },
      })
    })
    expect(screen.getByText('No sessions.')).toBeInTheDocument()
    unmount()
    expect(client.getState()).toBe('disconnected')
  })
})

const CATALOG = { harnesses: ['claude'], workdir_aliases: ['default'] }

describe('opening sessions', () => {
  it('opens a schema-valid session and observes creation before the response', async () => {
    const client = createFakeCoreClient(CATALOG)
    const send = vi.spyOn(client, 'send').mockImplementation((command) => {
      expect(isLocalCommand(command)).toBe(true)
      if (command.method === 'session.open') {
        const id = command.params.session_id as string
        client.emit({
          type: 'urn:ads:event:session.created:1',
          stream: `session/${id}`,
          data: { session_id: id, harness: 'claude', display_name: 'New session' },
        })
      }
      return Promise.resolve({
        jsonrpc: '2.0',
        id: command.id,
        result: { accepted: true, replayed: false },
      })
    })
    render(
      <I18nContext value={createTranslator('en')}>
        <App client={client} live />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', { name: 'Open claude' })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    await waitFor(() =>
      expect(
        screen.getByRole('region', { name: 'Terminal of session New session' }),
      ).toBeInTheDocument(),
    )
    expect(send.mock.calls[0]?.[0]).toMatchObject({
      method: 'session.open',
      params: { harness: 'claude', workdir_alias: 'default' },
    })
    expect(document.activeElement?.tagName).toBe('TEXTAREA')
  })
  it('shows a translated warning for a server rejection', async () => {
    const client = createFakeCoreClient(CATALOG)
    vi.spyOn(client, 'send').mockImplementation((command) =>
      Promise.resolve({
        jsonrpc: '2.0',
        id: command.id,
        error: { code: -1, message: 'private server details' },
      }),
    )
    const t = createTranslator('pt-BR')
    render(
      <I18nContext value={t}>
        <App client={client} live />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', { name: 'Abrir claude' })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    expect(await screen.findByRole('alert')).toHaveTextContent(t('session.commandFailed'))
    expect(screen.queryByText('private server details')).not.toBeInTheDocument()
  })
})

describe('tool list', () => {
  it('uses the tools and folder alias the core reports, ignoring the build-time list', async () => {
    const client = createFakeCoreClient({ harnesses: ['codex'], workdir_aliases: ['repo'] })
    const send = vi.spyOn(client, 'send')
    render(
      <I18nContext value={createTranslator('en')}>
        <App client={client} live harnesses={['claude']} workdir="default" />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', { name: 'Open codex' })
    expect(screen.queryByRole('button', { name: 'Open claude' })).not.toBeInTheDocument()
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    await waitFor(() => {
      expect(send).toHaveBeenCalled()
    })
    expect(send.mock.calls[0]?.[0]).toMatchObject({
      method: 'session.open',
      params: { harness: 'codex', workdir_alias: 'repo' },
    })
  })
  it('keeps the configured folder alias when the core knows it', async () => {
    const client = createFakeCoreClient({
      harnesses: ['claude'],
      workdir_aliases: ['other', 'chosen'],
    })
    const send = vi.spyOn(client, 'send')
    render(
      <I18nContext value={createTranslator('en')}>
        <App client={client} live workdir="chosen" />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', { name: 'Open claude' })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    await waitFor(() => {
      expect(send).toHaveBeenCalled()
    })
    expect(send.mock.calls[0]?.[0]).toMatchObject({ params: { workdir_alias: 'chosen' } })
  })
  it('shows no tools and a warning when the core refuses the list', async () => {
    const client = createFakeCoreClient(CATALOG)
    vi.spyOn(client, 'listHarnesses').mockRejectedValue(new Error('command rejected'))
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live harnesses={['claude']} />
      </I18nContext>,
    )
    expect(await screen.findByRole('alert')).toHaveTextContent(t('harness.failed'))
    expect(screen.queryByRole('button', { name: 'Open claude' })).not.toBeInTheDocument()
  })
  it('falls back to the build-time list only in sample mode', () => {
    const client = createFakeCoreClient(CATALOG)
    const listHarnesses = vi.spyOn(client, 'listHarnesses')
    render(
      <I18nContext value={createTranslator('en')}>
        <App client={client} harnesses={['sample-tool']} />
      </I18nContext>,
    )
    expect(screen.getByRole('button', { name: 'Open sample-tool' })).toBeDisabled()
    expect(listHarnesses).not.toHaveBeenCalled()
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

describe('tool list states', () => {
  it('tells a pending list apart from an empty one', async () => {
    const client = createFakeCoreClient(CATALOG)
    const answer = deferred<{ harnesses: string[]; workdir_aliases: string[] }>()
    vi.spyOn(client, 'listHarnesses').mockReturnValue(answer.promise)
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live />
      </I18nContext>,
    )
    expect(await screen.findByText(t('harness.loading'))).toBeInTheDocument()
    await act(async () => {
      answer.resolve({ harnesses: [], workdir_aliases: ['default'] })
      await answer.promise
    })
    expect(screen.getByText(t('harness.none'))).toBeInTheDocument()
    expect(screen.queryByText(t('harness.loading'))).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Open / })).not.toBeInTheDocument()
  })
  it('never opens a session without an alias the core announced', async () => {
    const client = createFakeCoreClient({ harnesses: ['claude'], workdir_aliases: [] })
    const send = vi.spyOn(client, 'send')
    const subscribe = vi.spyOn(client, 'subscribe')
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live workdir="default" />
      </I18nContext>,
    )
    expect(await screen.findByText(t('harness.noWorkdir'))).toBeInTheDocument()
    const button = screen.getByRole('button', { name: 'Open claude' })
    expect(button).toBeDisabled()
    fireEvent.click(button)
    expect(send).not.toHaveBeenCalled()
    expect(subscribe).not.toHaveBeenCalled()
  })
  it('uses only the answer of the current connection after reconnecting', async () => {
    const client = createFakeCoreClient(CATALOG)
    const first = deferred<{ harnesses: string[]; workdir_aliases: string[] }>()
    const second = deferred<{ harnesses: string[]; workdir_aliases: string[] }>()
    vi.spyOn(client, 'listHarnesses')
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const send = vi.spyOn(client, 'send')
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live />
      </I18nContext>,
    )
    await screen.findByText(t('harness.loading'))
    act(() => {
      client.disconnect()
    })
    await act(async () => {
      await client.connect()
    })
    expect(screen.getByText(t('harness.loading'))).toBeInTheDocument()
    await act(async () => {
      second.resolve({ harnesses: ['codex'], workdir_aliases: ['new'] })
      await second.promise
    })
    await act(async () => {
      first.resolve({ harnesses: ['claude'], workdir_aliases: ['old'] })
      await first.promise
    })
    expect(screen.queryByRole('button', { name: 'Open claude' })).not.toBeInTheDocument()
    const button = screen.getByRole('button', { name: 'Open codex' })
    await waitFor(() => {
      expect(button).toBeEnabled()
    })
    fireEvent.click(button)
    await waitFor(() => {
      expect(send).toHaveBeenCalled()
    })
    expect(send.mock.calls[0]?.[0]).toMatchObject({
      params: { harness: 'codex', workdir_alias: 'new' },
    })
  })
  it('releases the session subscription when a listed tool fails to open', async () => {
    const client = createFakeCoreClient(CATALOG)
    const stop = vi.fn()
    const subscribe = vi.spyOn(client, 'subscribe').mockReturnValue(stop)
    vi.spyOn(client, 'send').mockImplementation((command) =>
      Promise.resolve({
        jsonrpc: '2.0',
        id: command.id,
        error: { code: -32602, message: 'invalid command' },
      }),
    )
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', { name: 'Open claude' })
    await waitFor(() => {
      expect(button).toBeEnabled()
    })
    fireEvent.click(button)
    expect(await screen.findByRole('alert')).toHaveTextContent(t('session.commandFailed'))
    expect(subscribe).toHaveBeenCalledTimes(1)
    expect(stop).toHaveBeenCalledTimes(1)
  })
})
