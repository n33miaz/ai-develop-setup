import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createFakeCoreClient,
  HarnessListError,
  type CommandRequestV1,
  type CommandResponseV2,
  type FakeCoreClient,
  type SubscriptionEndCode,
} from './api'
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

/** Cliente cuja queda e volta chegam à tela como um só aviso, sem o estado desconectado no meio. */
function quietReconnect() {
  const fake = createFakeCoreClient(CATALOG)
  const listeners = new Set<(state: ReturnType<FakeCoreClient['getState']>) => void>()
  let muted = false
  fake.onStateChange((state) => {
    if (!muted) for (const listener of [...listeners]) listener(state)
  })
  const client: FakeCoreClient = {
    ...fake,
    onStateChange(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
  // `notify = false`: a tela nem fica sabendo da reconexão (renderização velha).
  const reconnect = async (notify = true) => {
    muted = true
    fake.disconnect()
    await fake.connect()
    muted = false
    if (notify) for (const listener of [...listeners]) listener(fake.getState())
  }
  return { client, reconnect }
}

describe('tool list across connections', () => {
  it('drops a loaded list when the client reconnects before the screen sees the drop', async () => {
    const { client, reconnect } = quietReconnect()
    const next = deferred<{ harnesses: string[]; workdir_aliases: string[] }>()
    vi.spyOn(client, 'listHarnesses')
      .mockResolvedValueOnce({ harnesses: ['claude'], workdir_aliases: ['old'] })
      .mockReturnValueOnce(next.promise)
    const send = vi.spyOn(client, 'send')
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live />
      </I18nContext>,
    )
    await screen.findByRole('button', { name: 'Open claude' })
    await act(async () => {
      await reconnect()
    })
    expect(screen.queryByRole('button', { name: 'Open claude' })).not.toBeInTheDocument()
    expect(screen.getByText(t('harness.loading'))).toBeInTheDocument()
    await act(async () => {
      next.resolve({ harnesses: ['codex'], workdir_aliases: ['new'] })
      await next.promise
    })
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
  it('ignores the failure of a query from the previous connection', async () => {
    const { client, reconnect } = quietReconnect()
    let failFirst!: (error: Error) => void
    const first = new Promise<never>((_, reject) => {
      failFirst = reject
    })
    const second = deferred<{ harnesses: string[]; workdir_aliases: string[] }>()
    vi.spyOn(client, 'listHarnesses').mockReturnValueOnce(first).mockReturnValueOnce(second.promise)
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live />
      </I18nContext>,
    )
    await screen.findByText(t('harness.loading'))
    await act(async () => {
      await reconnect()
    })
    await act(async () => {
      failFirst(new HarnessListError('timeout'))
      await first.catch(() => undefined)
    })
    expect(screen.queryByText(t('harness.timeout'))).not.toBeInTheDocument()
    expect(screen.getByText(t('harness.loading'))).toBeInTheDocument()
    await act(async () => {
      second.resolve({ harnesses: ['codex'], workdir_aliases: ['new'] })
      await second.promise
    })
    expect(screen.getByRole('button', { name: 'Open codex' })).toBeInTheDocument()
  })
})

describe('sessions the core cannot keep', () => {
  function created(client: FakeCoreClient, id: string, name: string) {
    client.emit({
      type: 'urn:ads:event:session.created:1',
      stream: `session/${id}`,
      data: { session_id: id, harness: 'claude', display_name: name },
    })
  }

  it('shows a pending close and sends session.close from its button', async () => {
    const client = createFakeCoreClient(CATALOG)
    const send = vi.spyOn(client, 'send')
    const t = createTranslator('pt-BR')
    render(
      <I18nContext value={t}>
        <App client={client} live sessionStreams={['session/session-a']} />
      </I18nContext>,
    )
    await waitFor(() => {
      expect(client.getState()).toBe('connected')
    })
    act(() => {
      created(client, 'session-a', 'Sessão A')
      client.end('session/session-a', -32013)
    })
    expect(screen.getByRole('button', { name: /Sessão A/ })).toHaveTextContent(
      t('session.state.closing'),
    )
    expect(screen.getByText(t('session.notice.closing'))).toBeInTheDocument()
    expect(client.getState()).toBe('connected')
    fireEvent.click(screen.getByRole('button', { name: t('session.close') }))
    await waitFor(() => {
      expect(screen.getByText(t('sessions.empty'))).toBeInTheDocument()
    })
    const command = send.mock.calls.at(-1)?.[0]
    expect(command).toMatchObject({
      method: 'session.close',
      params: { session_id: 'session-a' },
    })
    expect(command && isLocalCommand(command)).toBe(true)
    expect(screen.queryByText(t('session.notice.closing'))).not.toBeInTheDocument()
  })

  it('keeps the close button when the core refuses session.close', async () => {
    const client = createFakeCoreClient(CATALOG)
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live sessionStreams={['session/session-a']} />
      </I18nContext>,
    )
    await waitFor(() => {
      expect(client.getState()).toBe('connected')
    })
    act(() => {
      created(client, 'session-a', 'Session A')
      client.end('session/session-a', -32013)
    })
    vi.spyOn(client, 'send').mockImplementation((command) =>
      Promise.resolve({
        jsonrpc: '2.0',
        id: command.id,
        error: { code: -32603, message: 'service unavailable' },
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Close session' }))
    expect(await screen.findByRole('alert')).toHaveTextContent(t('session.commandFailed'))
    expect(screen.getByRole('button', { name: 'Close session' })).toBeInTheDocument()
  })

  it('marks a session whose subscription ended without losing the connection', async () => {
    const client = createFakeCoreClient(CATALOG)
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live sessionStreams={['session/session-a', 'session/session-b']} />
      </I18nContext>,
    )
    await waitFor(() => {
      expect(client.getState()).toBe('connected')
    })
    act(() => {
      created(client, 'session-a', 'Session A')
      created(client, 'session-b', 'Session B')
      client.end('session/session-a', -32004)
    })
    expect(screen.getByRole('button', { name: /Session A/ })).toHaveTextContent('Ended')
    expect(screen.getByText(t('session.notice.ended'))).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('session.close') })).not.toBeInTheDocument()
    expect(client.getState()).toBe('connected')
    act(() => {
      client.emit({
        type: 'urn:ads:event:session.activity_changed:1',
        stream: 'session/session-b',
        data: { session_id: 'session-b', activity: 'idle' },
      })
    })
    expect(screen.getByRole('button', { name: /Session B/ })).toHaveTextContent('Idle')
  })

  /**
   * Encerra cada assinatura de sessão individualmente, como o cliente real: o fim só chega a uma
   * assinatura ainda ativa e a desliga. Conta os cancelamentos feitos pela tela.
   */
  function trackSubscriptions(client: FakeCoreClient) {
    const ends: { stream: string; end: (code: SubscriptionEndCode) => void }[] = []
    const stopped: string[] = []
    const original = client.subscribe.bind(client)
    vi.spyOn(client, 'subscribe').mockImplementation((stream, afterSeq, onEvent, onEnd) => {
      const stop = original(stream, afterSeq, onEvent)
      let active = true
      if (onEnd)
        ends.push({
          stream,
          end: (code) => {
            if (!active) return
            active = false
            stop()
            onEnd(code)
          },
        })
      return () => {
        if (onEnd && active) stopped.push(stream)
        active = false
        stop()
      }
    })
    return { ends, stopped }
  }

  async function openWith(
    client: FakeCoreClient,
    answer: (command: CommandRequestV1) => Promise<CommandResponseV2>,
    t = createTranslator('en'),
    props: Partial<Parameters<typeof App>[0]> = {},
  ) {
    const send = vi.spyOn(client, 'send').mockImplementation(answer)
    render(
      <I18nContext value={t}>
        <App client={client} live harnesses={['claude']} {...props} />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', {
      name: t('session.open', { harness: 'claude' }),
    })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    await waitFor(() => expect(button).toBeEnabled())
    return send
  }

  function deferred() {
    let resolve: (response: CommandResponseV2) => void = () => undefined
    const promise = new Promise<CommandResponseV2>((done) => {
      resolve = done
    })
    return { promise, resolve }
  }

  const accepted = (command: CommandRequestV1 | undefined): CommandResponseV2 => ({
    jsonrpc: '2.0',
    id: command?.id ?? '',
    result: { accepted: true, replayed: false },
  })

  const refusal = (command: CommandRequestV1 | undefined): CommandResponseV2 => ({
    jsonrpc: '2.0',
    id: command?.id ?? '',
    error: { code: -32603, message: 'service unavailable' },
  })

  /** Sessões dos fluxos dados; as assinaturas nos índices de `closing` recebem -32013. */
  async function renderClosing(
    client: FakeCoreClient,
    ends: readonly { end: (code: SubscriptionEndCode) => void }[],
    ids: readonly string[],
    closing: readonly number[],
  ) {
    render(
      <I18nContext value={createTranslator('en')}>
        <App
          client={client}
          live
          sessionStreams={ids.map((id) => `session/${id}`)}
          closeTimeoutMs={50}
        />
      </I18nContext>,
    )
    await act(async () => {
      await Promise.resolve()
    })
    expect(client.getState()).toBe('connected')
    act(() => {
      for (const id of new Set(ids)) created(client, id, `Session ${id.slice(-1).toUpperCase()}`)
      for (const index of closing) ends[index]?.end(-32013)
    })
  }

  const refused = (command: CommandRequestV1, code: number) =>
    Promise.resolve<CommandResponseV2>({
      jsonrpc: '2.0',
      id: command.id,
      error: { code, message: 'refused' },
    })

  it('keeps the close action when -32013 arrives before -32014', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { ends } = trackSubscriptions(client)
    await openWith(client, (command) => {
      ends.at(-1)?.end(-32013)
      return refused(command, -32014)
    })
    expect(screen.getByRole('status', { name: 'Session state' })).toHaveTextContent(
      'The core could not close this session',
    )
    expect(screen.getByRole('button', { name: 'Close session' })).toBeInTheDocument()
  })

  it('shows the close action when -32013 arrives after -32014', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { ends } = trackSubscriptions(client)
    await openWith(client, (command) => refused(command, -32014))
    expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
    act(() => {
      ends.at(-1)?.end(-32013)
    })
    expect(screen.getByRole('button', { name: 'Close session' })).toBeInTheDocument()
  })

  it('does not bring back a closing session that another subscription saw end', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { ends } = trackSubscriptions(client)
    render(
      <I18nContext value={createTranslator('en')}>
        <App client={client} live sessionStreams={['session/session-a', 'session/session-a']} />
      </I18nContext>,
    )
    await waitFor(() => {
      expect(client.getState()).toBe('connected')
    })
    act(() => {
      created(client, 'session-a', 'Session A')
      ends[0]?.end(-32013)
    })
    expect(screen.getByRole('button', { name: 'Close session' })).toBeInTheDocument()
    act(() => {
      client.emit({
        type: 'urn:ads:event:session.ended:1',
        stream: 'session/session-a',
        data: { session_id: 'session-a', reason: 'exit', exit_code: 0 },
      })
    })
    expect(screen.getByText('No sessions.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
    act(() => {
      ends[1]?.end(-32013)
    })
    expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
  })

  it('releases the subscription of a refused open', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { stopped } = trackSubscriptions(client)
    await openWith(client, (command) => refused(command, -1))
    expect(stopped).toHaveLength(1)
  })

  it('releases the open subscription when the session ends after -32014', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { stopped } = trackSubscriptions(client)
    const send = await openWith(client, (command) => refused(command, -32014))
    expect(stopped).toHaveLength(0)
    const id = send.mock.calls[0]?.[0].params.session_id as string
    act(() => {
      client.emit({
        type: 'urn:ads:event:session.ended:1',
        stream: `session/${id}`,
        data: { session_id: id, reason: 'error' },
      })
    })
    expect(stopped).toHaveLength(1)
    expect(screen.queryByRole('button', { name: /Failed to start/ })).not.toBeInTheDocument()
  })

  it('shows the close action when another subscription of the stream ended first', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { ends } = trackSubscriptions(client)
    const t = createTranslator('en')
    const send = vi.spyOn(client, 'send')
    const { rerender } = render(
      <I18nContext value={t}>
        <App client={client} live harnesses={['claude']} />
      </I18nContext>,
    )
    const open = await screen.findByRole('button', { name: 'Open claude' })
    await waitFor(() => expect(open).toBeEnabled())
    fireEvent.click(open)
    await waitFor(() => expect(open).toBeEnabled())
    const id = send.mock.calls[0]?.[0].params.session_id as string
    rerender(
      <I18nContext value={t}>
        <App client={client} live harnesses={['claude']} sessionStreams={[`session/${id}`]} />
      </I18nContext>,
    )
    expect(ends).toHaveLength(2)
    act(() => {
      ends[1]?.end(-32004)
      ends[0]?.end(-32013)
    })
    expect(screen.getByRole('button', { name: 'Close session' })).toBeInTheDocument()
  })

  it('shows a new session that reuses the id of a closed one', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { ends } = trackSubscriptions(client)
    await renderClosing(client, ends, ['session-a', 'session-a'], [0])
    fireEvent.click(screen.getByRole('button', { name: 'Close session' }))
    await waitFor(() => {
      expect(screen.getByText('No sessions.')).toBeInTheDocument()
    })
    act(() => {
      ends[0]?.end(-32013)
      created(client, 'session-a', 'Session A again')
    })
    expect(screen.getByRole('button', { name: /Session A again/ })).toHaveTextContent('Active')
    expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
  })

  describe('with a controlled clock', () => {
    afterEach(() => {
      vi.useRealTimers()
    })

    const closes = (send: { mock: { calls: [CommandRequestV1][] } }) =>
      send.mock.calls.map(([command]) => command).filter((c) => c.method === 'session.close')

    it('completes a close confirmed after the deadline without a stale warning', async () => {
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      const client = createFakeCoreClient(CATALOG)
      const { ends } = trackSubscriptions(client)
      await renderClosing(client, ends, ['session-a'], [0])
      const answer = deferred()
      const send = vi.spyOn(client, 'send').mockImplementation(() => answer.promise)
      fireEvent.click(screen.getByRole('button', { name: 'Close session' }))
      await act(() => vi.advanceTimersByTimeAsync(50))
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Close session' })).toBeEnabled()
      await act(async () => {
        answer.resolve(accepted(closes(send)[0]))
        await Promise.resolve()
      })
      expect(screen.getByText('No sessions.')).toBeInTheDocument()
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    })

    it('reuses the key and ignores a stale refusal after the retry confirms', async () => {
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      const client = createFakeCoreClient(CATALOG)
      const { ends } = trackSubscriptions(client)
      await renderClosing(client, ends, ['session-a'], [0])
      const answers = [deferred(), deferred()]
      let call = 0
      const send = vi
        .spyOn(client, 'send')
        .mockImplementation(() => (answers[call++] ?? deferred()).promise)
      fireEvent.click(screen.getByRole('button', { name: 'Close session' }))
      await act(() => vi.advanceTimersByTimeAsync(50))
      fireEvent.click(screen.getByRole('button', { name: 'Close session' }))
      const [first, retry] = closes(send)
      expect(retry?.params._meta.idempotency_key).toBe(first?.params._meta.idempotency_key)
      expect(retry?.id).not.toBe(first?.id)
      await act(async () => {
        answers[1]?.resolve(accepted(retry))
        await Promise.resolve()
      })
      expect(screen.getByText('No sessions.')).toBeInTheDocument()
      await act(async () => {
        answers[0]?.resolve(refusal(first))
        await vi.advanceTimersByTimeAsync(100)
      })
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
      expect(screen.getByText('No sessions.')).toBeInTheDocument()
    })

    it('sends one close per session while it waits and keeps other sessions closable', async () => {
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      const client = createFakeCoreClient(CATALOG)
      const { ends } = trackSubscriptions(client)
      await renderClosing(client, ends, ['session-a', 'session-b'], [0, 1])
      const send = vi.spyOn(client, 'send').mockImplementation(() => new Promise(() => undefined))
      const closeA = screen.getByRole('button', { name: 'Close session' })
      // Os dois cliques chegam antes de a tela atualizar o botão.
      act(() => {
        closeA.click()
        closeA.click()
      })
      fireEvent.click(screen.getByRole('button', { name: /Session B/ }))
      const closeB = screen.getByRole('button', { name: 'Close session' })
      expect(closeB).toBeEnabled()
      fireEvent.click(closeB)
      expect(closes(send).map((c) => c.params.session_id)).toEqual(['session-a', 'session-b'])
      await act(() => vi.advanceTimersByTimeAsync(50))
      expect(screen.getByRole('alert')).toHaveTextContent(
        createTranslator('en')('session.commandFailed'),
      )
      fireEvent.click(screen.getByRole('button', { name: /Session A/ }))
      expect(screen.getByRole('button', { name: 'Close session' })).toBeEnabled()
    })

    it('keeps focus on another session when a pending close completes', async () => {
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      const client = createFakeCoreClient(CATALOG)
      const { ends } = trackSubscriptions(client)
      await renderClosing(client, ends, ['session-a', 'session-b'], [0, 1])
      const answer = deferred()
      const send = vi.spyOn(client, 'send').mockImplementation(() => answer.promise)
      fireEvent.click(screen.getByRole('button', { name: 'Close session' }))
      fireEvent.click(screen.getByRole('button', { name: /Session B/ }))
      const closeB = screen.getByRole('button', { name: 'Close session' })
      closeB.focus()
      await act(async () => {
        answer.resolve(accepted(closes(send)[0]))
        await Promise.resolve()
      })
      expect(screen.queryByRole('button', { name: /Session A/ })).not.toBeInTheDocument()
      expect(document.activeElement).toBe(closeB)
    })
  })

  it('moves focus to the terminal when the session ends under the focused button', async () => {
    const client = createFakeCoreClient(CATALOG)
    const { ends } = trackSubscriptions(client)
    await renderClosing(client, ends, ['session-a', 'session-a'], [0])
    const region = screen.getByRole('status', { name: 'Session state' })
    screen.getByRole('button', { name: 'Close session' }).focus()
    act(() => {
      client.emit({
        type: 'urn:ads:event:session.ended:1',
        stream: 'session/session-a',
        data: { session_id: 'session-a', reason: 'exit', exit_code: 0 },
      })
    })
    expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
    expect(document.activeElement).toBe(document.getElementById('terminal'))
    expect(region).toBeEmptyDOMElement()
  })

  it('announces the notice in a live region and moves focus after closing', async () => {
    const client = createFakeCoreClient(CATALOG)
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live sessionStreams={['session/session-a']} />
      </I18nContext>,
    )
    await waitFor(() => {
      expect(client.getState()).toBe('connected')
    })
    const region = screen.getByRole('status', { name: 'Session state' })
    expect(region).toBeEmptyDOMElement()
    act(() => {
      created(client, 'session-a', 'Session A')
      client.end('session/session-a', -32013)
    })
    expect(screen.getByRole('status', { name: 'Session state' })).toBe(region)
    expect(region).toHaveTextContent(t('session.notice.closing'))
    const button = screen.getByRole('button', { name: 'Close session' })
    expect(region).not.toContainElement(button)
    button.focus()
    fireEvent.click(button)
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
    })
    expect(document.activeElement).toBe(document.getElementById('terminal'))
    expect(region).toBeEmptyDOMElement()
  })

  it('shows a session that timed out while starting', async () => {
    const client = createFakeCoreClient(CATALOG)
    vi.spyOn(client, 'send').mockImplementation((command) =>
      Promise.resolve({
        jsonrpc: '2.0',
        id: command.id,
        error: { code: -32014, message: 'session setup timed out' },
      }),
    )
    const t = createTranslator('pt-BR')
    render(
      <I18nContext value={t}>
        <App client={client} live harnesses={['claude']} />
      </I18nContext>,
    )
    const button = await screen.findByRole('button', { name: 'Abrir claude' })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    expect(await screen.findByText(t('session.notice.setupFailed'))).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Falhou ao iniciar/ })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})

describe('session streams across connections', () => {
  it('subscribes the session streams again after a reconnect the screen did not see', async () => {
    const { client, reconnect } = quietReconnect()
    const t = createTranslator('en')
    render(
      <I18nContext value={t}>
        <App client={client} live sessionStreams={['session/session-a']} />
      </I18nContext>,
    )
    await screen.findByRole('button', { name: 'Open claude' })
    await act(async () => {
      await reconnect()
    })
    act(() => {
      for (const draft of sampleSessionEvents(t)) {
        if (draft.stream === 'session/session-a') client.emit(draft)
      }
    })
    expect(
      screen.getByRole('region', { name: t('terminal.label', { name: t('sample.session.a') }) }),
    ).toBeInTheDocument()
  })
  it('does not open from a button of a connection that was already replaced', async () => {
    const { client, reconnect } = quietReconnect()
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
    const subscribe = vi.spyOn(client, 'subscribe')
    const send = vi.spyOn(client, 'send')
    await reconnect(false)
    fireEvent.click(button)
    await act(async () => {
      await Promise.resolve()
    })
    expect(subscribe).not.toHaveBeenCalled()
    expect(send).not.toHaveBeenCalled()
  })
})
