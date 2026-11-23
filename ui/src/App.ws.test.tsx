import { act, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createWsCoreClient, type CommandRequestV1 } from './api'
import { App } from './App'
import { createTranslator, I18nContext, type Locale } from './i18n'

class Socket extends EventTarget {
  sent: CommandRequestV1[] = []
  send(text: string) {
    this.sent.push(JSON.parse(text) as CommandRequestV1)
  }
  close() {
    this.dispatchEvent(new Event('close'))
  }
  reply(message: unknown) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(message) }))
  }
}

function renderSocket(listTimeoutMs: number, locale: Locale = 'en') {
  const socket = new Socket()
  const client = createWsCoreClient({
    url: 'ws://127.0.0.1:12345',
    token: 'a'.repeat(43),
    listTimeoutMs,
    socketFactory: () => socket,
  })
  const t = createTranslator(locale)
  render(
    <I18nContext value={t}>
      <App client={client} live harnesses={['claude']} />
    </I18nContext>,
  )
  socket.dispatchEvent(new Event('open'))
  return { socket, t }
}

async function mount(listTimeoutMs: number) {
  const { socket, t } = renderSocket(listTimeoutMs)
  socket.reply({
    jsonrpc: '2.0',
    id: socket.sent[0]?.id,
    result: { accepted: true, replayed: false },
  })
  await waitFor(() => {
    expect(socket.sent[1]?.method).toBe('harness.list')
  })
  return { socket, t }
}

describe('browser connection guidance', () => {
  it.each<Locale>(['en', 'pt-BR'])('explains a rejected launch link in %s', async (locale) => {
    const { socket, t } = renderSocket(10000, locale)
    socket.reply({
      jsonrpc: '2.0',
      id: socket.sent[0]?.id,
      error: { code: -32600, message: 'rejected' },
    })
    const guidance = locale === 'en' ? /Open the new full link/ : /Abra o novo link completo/
    expect(await screen.findByRole('status', { name: guidance })).toHaveAttribute(
      'data-state',
      'link_unavailable',
    )
    expect(screen.queryByText(t('notice.live'))).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: t('session.open', { harness: 'claude' }) }),
    ).not.toBeInTheDocument()
  })

  it('replaces the connected notice when the web connection drops', async () => {
    const { socket, t } = await mount(10000)
    expect(screen.getByText(t('notice.live'))).toBeInTheDocument()
    act(() => {
      socket.close()
    })
    expect(screen.getByRole('status', { name: /Open the new full link/ })).toBeInTheDocument()
    expect(screen.queryByText(t('notice.live'))).not.toBeInTheDocument()
  })
})

describe('tool list over the local socket', () => {
  it('leaves the loading state with a timeout notice when the core stays silent', async () => {
    const { t } = await mount(30)
    expect(screen.getByText(t('harness.loading'))).toBeInTheDocument()
    expect(await screen.findByRole('alert')).toHaveTextContent(t('harness.timeout'))
    expect(screen.queryByText(t('harness.loading'))).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Open claude' })).not.toBeInTheDocument()
  })
  it('explains that an older core does not report its tools', async () => {
    const { socket, t } = await mount(10000)
    socket.reply({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32601, message: 'unknown method' },
    })
    expect(await screen.findByRole('alert')).toHaveTextContent(t('harness.unsupported'))
    expect(screen.queryByText(t('harness.timeout'))).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Open claude' })).not.toBeInTheDocument()
  })
})

describe('tool list refusal over the local socket', () => {
  it('shows the neutral refusal notice for an error other than an unknown method', async () => {
    const { socket, t } = await mount(10000)
    socket.reply({
      jsonrpc: '2.0',
      id: socket.sent[1]?.id,
      error: { code: -32602, message: 'invalid command' },
    })
    expect(await screen.findByRole('alert')).toHaveTextContent(t('harness.failed'))
    expect(screen.queryByText(t('harness.unsupported'))).not.toBeInTheDocument()
  })
})
