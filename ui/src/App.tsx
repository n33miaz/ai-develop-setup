import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react'

import {
  decodeBase64,
  newCommand,
  newUlid,
  SESSION_CLOSE_PENDING,
  SESSION_SETUP_TIMED_OUT,
  useConnectionState,
  type CoreClient,
  type EventEnvelopeV1,
  type SubscriptionEndCode,
} from './api'

import { sessionInput } from './api/session'

import styles from './App.module.css'
import { ConnectionStatus } from './components/ConnectionStatus'
import { Sidebar } from './components/Sidebar/Sidebar'
import { eventByteSource, staticByteSource } from './components/Terminal/byteSource'
import { Terminal } from './components/Terminal/Terminal'
import { useT, type MessageKey, type Translate } from './i18n'
import { sessionsFrom, tasksFrom, type SessionStatus, type SessionView } from './model/views'
import { sampleEvents, sampleOutput, type SampleEvents } from './sample/data'
import type { ThemePreference } from './theme/theme'
import { useTheme } from './theme/useTheme'
import type { SessionEvent } from './api'
import { UpdateNotice } from './pwa/UpdateNotice'

const NO_STREAMS: readonly string[] = []

const THEME_LABEL: Record<ThemePreference, MessageKey> = {
  system: 'theme.system',
  light: 'theme.light',
  dark: 'theme.dark',
}

const SESSION_NOTICE: Record<SessionStatus, MessageKey> = {
  closing: 'session.notice.closing',
  ended: 'session.notice.ended',
  setupFailed: 'session.notice.setupFailed',
}

/** Estado de cada sessão que não vem de evento. `closed`: o `session.close` foi aceito. */
type SessionStatuses = Readonly<Record<string, SessionStatus | 'closed'>>

function nextStatus(
  current: SessionStatus | 'closed' | undefined,
  next: SessionStatus | 'closed',
): SessionStatus | 'closed' | undefined {
  // Fechada fica fechada; o fim de outra assinatura não esconde um fechamento ainda pendente.
  if (current === 'closed') return current
  if (current === 'closing' && next === 'ended') return current
  return next
}

/** Junta o estado local às sessões dos eventos; fechamento pendente ou falha aparecem mesmo sem `session.created`. */
function withStatuses(sessions: SessionView[], statuses: SessionStatuses): SessionView[] {
  const views: SessionView[] = []
  for (const session of sessions) {
    const status = statuses[session.id]
    if (status === 'closed') continue
    views.push(status ? { ...session, status } : session)
  }
  for (const [id, status] of Object.entries(statuses)) {
    if (status !== 'closing' && status !== 'setupFailed') continue
    if (!sessions.some((session) => session.id === id)) views.push({ id, activity: 'idle', status })
  }
  return views
}

const TERMINAL_ID = 'terminal'
const COMPACT_QUERY = '(width < 48rem)'

function subscribeCompact(onChange: () => void) {
  const media = window.matchMedia(COMPACT_QUERY)
  media.addEventListener('change', onChange)
  return () => {
    media.removeEventListener('change', onChange)
  }
}

function isCompact() {
  return window.matchMedia(COMPACT_QUERY).matches
}

interface AppProps {
  client: CoreClient
  live?: boolean

  harnesses?: readonly string[]

  workdir?: string

  resizeDebounceMs?: number

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

  harnesses = NO_STREAMS,

  workdir = 'default',

  resizeDebounceMs = 150,
}: AppProps) {
  const t = useT()
  const sidebarId = useId()
  const connection = useConnectionState(client)
  const { preference, cycle } = useTheme()
  const compact = useSyncExternalStore(subscribeCompact, isCompact)
  const [menuOpen, setMenuOpen] = useState(false)

  const [warning, setWarning] = useState(false)

  const reportError = useCallback(() => {
    setWarning(true)
  }, [])

  const openedSubscriptions = useRef<(() => void)[]>([])

  const size = useRef({ cols: 80, rows: 24 })

  const [opening, setOpening] = useState(false)

  const [liveEvents, setLiveEvents] = useState<SessionEvent[]>([])

  const [statuses, setStatuses] = useState<SessionStatuses>({})

  const [closing, setClosing] = useState(false)

  const markSession = useCallback((id: string, status: SessionStatus | 'closed') => {
    setStatuses((previous) => {
      const next = nextStatus(previous[id], status)
      return next === previous[id] || next === undefined ? previous : { ...previous, [id]: next }
    })
  }, [])

  // Só a assinatura termina: a conexão continua e as outras sessões seguem recebendo eventos.
  const endSession = useCallback(
    (stream: string) => (code: SubscriptionEndCode) => {
      if (!stream.startsWith('session/')) return
      markSession(
        stream.slice('session/'.length),
        code === SESSION_CLOSE_PENDING ? 'closing' : 'ended',
      )
    },
    [markSession],
  )

  const receiveSession = useCallback((event: EventEnvelopeV1) => {
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
  }, [])

  useEffect(
    () => () => {
      for (const stop of openedSubscriptions.current) stop()

      openedSubscriptions.current = []
    },
    [client, connection],
  )

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
      client.subscribe(stream, 0, receiveSession, endSession(stream)),
    )

    return () => {
      for (const stop of unsubscribe) stop()
    }
  }, [client, live, connection, sessionStreams, receiveSession, endSession])

  const { tasks, sessions } = useMemo(() => {
    if (live) return { tasks: [], sessions: withStatuses(sessionsFrom(liveEvents), statuses) }
    const replayed = events(t)
    return { tasks: tasksFrom(replayed.tasks), sessions: sessionsFrom(replayed.sessions) }
  }, [events, t, live, liveEvents, statuses])
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

  const input = useMemo(
    () =>
      live && sessionId && connection === 'connected'
        ? sessionInput(client, sessionId, reportError)
        : undefined,

    [live, sessionId, connection, client, reportError],
  )

  const resize = useCallback(
    (cols: number, rows: number) => {
      size.current = { cols: Math.min(1000, cols), rows: Math.min(1000, rows) }

      if (!input || !sessionId) return

      void client
        .send(newCommand('session.resize', { session_id: sessionId, ...size.current }))

        .then((response) => {
          if (response.error) reportError()
        }, reportError)
    },
    [client, input, sessionId, reportError],
  )

  const open = async (harness: string) => {
    setOpening(true)

    setWarning(false)

    const id = `s-${newUlid()}`

    // Assina antes da abertura: session.created pode preceder a resposta.

    setChosenSessionId(id)

    try {
      const stream = `session/${id}`
      const stop = client.subscribe(stream, 0, receiveSession, endSession(stream))

      openedSubscriptions.current.push(stop)

      const response = await client.send(
        newCommand('session.open', {
          session_id: id,
          harness,
          workdir_alias: workdir,
          ...size.current,
        }),
      )

      if (response.error?.code === SESSION_SETUP_TIMED_OUT) markSession(id, 'setupFailed')
      else if (response.error) reportError()
    } catch {
      reportError()
    } finally {
      setOpening(false)
    }
  }

  const close = async (id: string) => {
    setClosing(true)

    setWarning(false)

    try {
      const response = await client.send(newCommand('session.close', { session_id: id }))

      if (response.error) reportError()
      else markSession(id, 'closed')
    } catch {
      reportError()
    } finally {
      setClosing(false)
    }
  }

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

      {live && harnesses.length > 0 && (
        <div className={styles.actions}>
          {harnesses.map((harness) => (
            <button
              key={harness}
              type="button"
              className={styles.button}
              disabled={connection !== 'connected' || opening}
              onClick={() => {
                void open(harness)
              }}
            >
              {t('session.open', { harness })}
            </button>
          ))}
        </div>
      )}

      {warning && <p role="alert">{t('session.commandFailed')}</p>}

      {session?.status && (
        <div className={styles.status} role="status">
          <p>{t(SESSION_NOTICE[session.status])}</p>
          {session.status === 'closing' && (
            <button
              type="button"
              className={styles.button}
              disabled={connection !== 'connected' || closing}
              onClick={() => {
                void close(session.id)
              }}
            >
              {t('session.close')}
            </button>
          )}
        </div>
      )}

      <p className={styles.notice}>{t(live ? 'notice.live' : 'notice.wip')}</p>
      <UpdateNotice className={styles.notice} />
      <div className={styles.body}>
        <Sidebar
          id={sidebarId}
          open={menuOpen}
          hidden={compact && !menuOpen}
          tasks={tasks}
          sessions={sessions}
          selectedSessionId={sessionId ?? null}
          onSelectSession={(id) => {
            setChosenSessionId(id)
            setMenuOpen(false)
            if (compact) document.getElementById(TERMINAL_ID)?.focus()
          }}
        />
        <main id={TERMINAL_ID} className={styles.main} tabIndex={-1}>
          <Terminal
            source={source}
            label={t('terminal.label', { name: sessionName })}
            onInput={input}
            onResize={resize}
            resizeDebounceMs={resizeDebounceMs}
          />
        </main>
      </div>
    </div>
  )
}
