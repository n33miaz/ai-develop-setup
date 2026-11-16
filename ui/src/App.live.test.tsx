import { act, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { describe, expect, it } from 'vitest'
import { createFakeCoreClient } from './api'
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
