import { useEffect, useId, useMemo, useState } from 'react'
import { decodeBase64, useConnectionState, type CoreClient } from './api'
import styles from './App.module.css'
import { ConnectionStatus } from './components/ConnectionStatus'
import { Sidebar } from './components/Sidebar/Sidebar'
import { eventByteSource, staticByteSource } from './components/Terminal/byteSource'
import { Terminal } from './components/Terminal/Terminal'
import { useT, type MessageKey, type Translate } from './i18n'
import { sessionsFrom, tasksFrom } from './model/views'
import { sampleEvents, sampleOutput, type SampleEvents } from './sample/data'
import type { ThemePreference } from './theme/theme'
import { useTheme } from './theme/useTheme'
import type { SessionEvent } from './api'

const NO_STREAMS: readonly string[] = []

const THEME_LABEL: Record<ThemePreference, MessageKey> = {
  system: 'theme.system',
  light: 'theme.light',
  dark: 'theme.dark',
}

const TERMINAL_ID = 'terminal'

interface AppProps {
  client: CoreClient
  live?: boolean
  sessionStreams?: readonly string[]
  /** Os eventos que a tela reproduz; enquanto o núcleo não a alimenta, os de exemplo. */
  events?: (t: Translate) => SampleEvents
}

/** Tela "em construção": a estrutura final (barra lateral, terminal, conexão) com dados de exemplo. */
export function App({
  client,
  events = sampleEvents,
  live = false,
  sessionStreams = NO_STREAMS,
}: AppProps) {
  const t = useT()
  const sidebarId = useId()
  const connection = useConnectionState(client)
  const { preference, cycle } = useTheme()
  const [menuOpen, setMenuOpen] = useState(false)
  const [liveEvents, setLiveEvents] = useState<SessionEvent[]>([])
  useEffect(() => {
    if (!live) return
    void client.connect().catch(() => undefined)
    return () => {
      client.disconnect()
    }
  }, [client, live])
  useEffect(() => {
    if (!live || connection !== 'connected') return
    const unsubscribe = sessionStreams.map((stream) =>
      client.subscribe(stream, 0, (event) => {
        if (
          ![
            'urn:ads:event:session.created:1',
            'urn:ads:event:session.activity_changed:1',
            'urn:ads:event:session.ended:1',
          ].includes(event.type)
        )
          return
        setLiveEvents((previous) =>
          previous.some((item) => item.id === event.id)
            ? previous
            : [...previous, event as SessionEvent],
        )
      }),
    )
    return () => {
      for (const stop of unsubscribe) stop()
    }
  }, [client, live, connection, sessionStreams])
  const { tasks, sessions } = useMemo(() => {
    if (live) return { tasks: [], sessions: sessionsFrom(liveEvents) }
    const replayed = events(t)
    return { tasks: tasksFrom(replayed.tasks), sessions: sessionsFrom(replayed.sessions) }
  }, [events, t, live, liveEvents])
  const [chosenSessionId, setChosenSessionId] = useState<string | null>(null)

  // Seleção sempre válida: a escolhida enquanto existir, senão a primeira, senão nenhuma.
  const session = sessions.find((item) => item.id === chosenSessionId) ?? sessions[0]
  const sessionId = session?.id
  const sessionName = session ? (session.displayName ?? t('session.unnamed')) : ''
  const source = useMemo(
    () =>
      live && sessionId !== undefined && connection === 'connected'
        ? eventByteSource(client, `session/${sessionId}`)
        : staticByteSource(
            live
              ? ''
              : sessionId === undefined
                ? ''
                : decodeBase64(sampleOutput(t, sessionId, sessionName).data.payload),
          ),
    [t, sessionId, sessionName, live, client, connection],
  )

  return (
    <div className={styles.app}>
      <a className={styles.skip} href={`#${TERMINAL_ID}`}>
        {t('skip.toTerminal')}
      </a>
      <header className={styles.header}>
        <button
          type="button"
          className={[styles.button, styles.menu].join(' ')}
          aria-expanded={menuOpen}
          aria-controls={sidebarId}
          onClick={() => {
            setMenuOpen((open) => !open)
          }}
        >
          {t('nav.menu')}
        </button>
        <h1 className={styles.title}>{t('app.title')}</h1>
        <ConnectionStatus state={connection} />
        <button type="button" className={styles.button} onClick={cycle}>
          {t('theme.button', { mode: t(THEME_LABEL[preference]) })}
        </button>
      </header>
      <p className={styles.notice}>{t(live ? 'notice.live' : 'notice.wip')}</p>
      <div className={styles.body}>
        <Sidebar
          id={sidebarId}
          open={menuOpen}
          tasks={tasks}
          sessions={sessions}
          selectedSessionId={sessionId ?? null}
          onSelectSession={(id) => {
            setChosenSessionId(id)
            setMenuOpen(false)
          }}
        />
        <main id={TERMINAL_ID} className={styles.main} tabIndex={-1}>
          <Terminal source={source} label={t('terminal.label', { name: sessionName })} />
        </main>
      </div>
    </div>
  )
}
