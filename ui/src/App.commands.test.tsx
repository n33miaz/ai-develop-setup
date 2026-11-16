import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { App } from './App'
import { createFakeCoreClient, decodeBase64 } from './api'
import { isLocalCommand } from './api/wire'
import { createTranslator, I18nContext } from './i18n'
import { sampleSessionEvents } from './sample/data'

vi.mock('./components/Terminal/Terminal', () => ({
  Terminal: ({
    onInput,
    onResize,
  }: {
    onInput?: (bytes: Uint8Array) => void
    onResize?: (cols: number, rows: number) => void
  }) => (
    <>
      <button onClick={() => onInput?.(new TextEncoder().encode('á😀\r\x03'))}>input</button>
      <button onClick={() => onResize?.(130, 45)}>resize</button>
    </>
  ),
}))

describe('live terminal commands', () => {
  it('routes input and fitted dimensions to the selected session', async () => {
    const client = createFakeCoreClient()
    const t = createTranslator('en')
    for (const draft of sampleSessionEvents(t)) client.emit(draft)
    const send = vi.spyOn(client, 'send')
    render(
      <I18nContext value={t}>
        <App client={client} live sessionStreams={['session/session-a']} />
      </I18nContext>,
    )
    await screen.findByText(t('sample.session.a'))
    fireEvent.click(screen.getByText('input'))
    fireEvent.click(screen.getByText('resize'))
    await waitFor(() => {
      expect(send).toHaveBeenCalledTimes(2)
    })
    const commands = send.mock.calls.map(([command]) => command)
    for (const command of commands) expect(isLocalCommand(command)).toBe(true)
    const write = commands.find((command) => command.method === 'session.write')
    if (!write) throw new Error('missing session write')
    expect(write.params.session_id).toBe('session-a')
    expect(new TextDecoder().decode(decodeBase64(write.params.data as string))).toBe('á😀\r\x03')
    expect(commands.find((command) => command.method === 'session.resize')).toMatchObject({
      params: { session_id: 'session-a', cols: 130, rows: 45 },
    })
  })
})
