import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createFakeCoreClient,
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

describe('opening sessions', () => {
  it('opens a schema-valid session and observes creation before the response', async () => {
    const client = createFakeCoreClient()
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
        <App client={client} live harnesses={['claude']} />
      </I18nContext>,
    )
    const button = screen.getByRole('button', { name: 'Open claude' })
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
    const client = createFakeCoreClient()
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
        <App client={client} live harnesses={['claude']} />
      </I18nContext>,
    )
    const button = screen.getByRole('button', { name: 'Abrir claude' })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    expect(await screen.findByRole('alert')).toHaveTextContent(t('session.commandFailed'))
    expect(screen.queryByText('private server details')).not.toBeInTheDocument()
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
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
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
    const button = screen.getByRole('button', { name: t('session.open', { harness: 'claude' }) })
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
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
    const { ends } = trackSubscriptions(client)
    await openWith(client, (command) => refused(command, -32014))
    expect(screen.queryByRole('button', { name: 'Close session' })).not.toBeInTheDocument()
    act(() => {
      ends.at(-1)?.end(-32013)
    })
    expect(screen.getByRole('button', { name: 'Close session' })).toBeInTheDocument()
  })

  it('does not bring back a closing session that another subscription saw end', async () => {
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
    const { stopped } = trackSubscriptions(client)
    await openWith(client, (command) => refused(command, -1))
    expect(stopped).toHaveLength(1)
  })

  it('releases the open subscription when the session ends after -32014', async () => {
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
    const { ends } = trackSubscriptions(client)
    const t = createTranslator('en')
    const send = vi.spyOn(client, 'send')
    const { rerender } = render(
      <I18nContext value={t}>
        <App client={client} live harnesses={['claude']} />
      </I18nContext>,
    )
    const open = screen.getByRole('button', { name: 'Open claude' })
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
    const client = createFakeCoreClient()
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
      const client = createFakeCoreClient()
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
      const client = createFakeCoreClient()
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
      const client = createFakeCoreClient()
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
      const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
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
    const client = createFakeCoreClient()
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
    const button = screen.getByRole('button', { name: 'Abrir claude' })
    await waitFor(() => expect(button).toBeEnabled())
    fireEvent.click(button)
    expect(await screen.findByText(t('session.notice.setupFailed'))).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Falhou ao iniciar/ })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})
