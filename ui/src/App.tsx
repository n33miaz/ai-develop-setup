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
  HarnessListError,
  SESSION_CLOSE_PENDING,
  SESSION_SETUP_TIMED_OUT,
  useConnectionGeneration,
  useConnectionState,
  type CommandRequestV1,
  type CoreClient,
  type HarnessListFailure,
  type EventEnvelopeV1,
  type HarnessCatalog,
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

const FAILURE_NOTICE: Record<HarnessListFailure, MessageKey> = {
  timeout: 'harness.timeout',
  unsupported: 'harness.unsupported',
  refused: 'harness.failed',
}
const FAILURE_NOTICES = new Set<MessageKey>(Object.values(FAILURE_NOTICE))

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
  // Fechada fica fechada; nada que chegue depois esconde um fechamento que ainda depende do usuário.
  if (current === 'closed') return current
  if (current === 'closing' && next !== 'closed') return current
  return next
}

function statusAfter(code: SubscriptionEndCode): SessionStatus {
  return code === SESSION_CLOSE_PENDING ? 'closing' : 'ended'
}

function without(statuses: SessionStatuses, id: string): SessionStatuses {
  return Object.fromEntries(Object.entries(statuses).filter(([key]) => key !== id))
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
  /** Reserva do modo de exemplo; conectada, a interface usa a lista do núcleo. */
  harnesses?: readonly string[]

  workdir?: string

  resizeDebounceMs?: number

  /** Quanto esperar a resposta de `session.close`; depois disso o botão volta e a repetição usa a mesma chave. */
  closeTimeoutMs?: number

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

  closeTimeoutMs = 10000,
}: AppProps) {
  const t = useT()
  const sidebarId = useId()
  const connection = useConnectionState(client)
  const generation = useConnectionGeneration(client)
  const { preference, cycle } = useTheme()
  const compact = useSyncExternalStore(subscribeCompact, isCompact)
  const [menuOpen, setMenuOpen] = useState(false)

  // Quem levantou o aviso: o fechamento de uma sessão só apaga o aviso que ele mesmo levantou.
  const [warning, setWarning] = useState<string | null>(null)

  const reportError = useCallback(() => {
    setWarning('command')
  }, [])

  const mainRef = useRef<HTMLElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)

  // Assinaturas abertas por `open`, por sessão, até terminarem ou a sessão acabar.
  const openedSubscriptions = useRef(new Map<string, { stop: () => void }>())

  const releaseOpened = useCallback((id: string) => {
    const entry = openedSubscriptions.current.get(id)
    openedSubscriptions.current.delete(id)
    entry?.stop()
  }, [])

  // A tentativa de fechamento em curso, por sessão (trava síncrona), e o comando cujo resultado
  // ainda é incerto: repetir usa a mesma chave de idempotência.
  const closeLocks = useRef(new Map<string, CommandRequestV1>())
  const closeCommands = useRef(new Map<string, CommandRequestV1>())

  // Instâncias que o núcleo deu por encerradas: avisos tardios delas não valem mais.
  const finished = useRef(new Set<string>())
  const seenEvents = useRef(new Set<string>())

  const size = useRef({ cols: 80, rows: 24 })

  const [opening, setOpening] = useState(false)

  const [liveEvents, setLiveEvents] = useState<SessionEvent[]>([])

  // Vale só para a conexão autenticada (geração) que respondeu.
  const [catalog, setCatalog] = useState<{
    generation: number
    value: HarnessCatalog | HarnessListFailure
  } | null>(null)

  const [statuses, setStatuses] = useState<SessionStatuses>({})

  const [closingIds, setClosingIds] = useState<ReadonlySet<string>>(new Set())

  const unlockClose = useCallback((id: string, command?: CommandRequestV1) => {
    if (command && closeLocks.current.get(id) !== command) return
    if (!closeLocks.current.delete(id)) return
    setClosingIds(new Set(closeLocks.current.keys()))
  }, [])

  const markSession = useCallback((id: string, status: SessionStatus) => {
    if (finished.current.has(id)) return
    setStatuses((previous) => {
      const next = nextStatus(previous[id], status)
      return next === previous[id] || next === undefined ? previous : { ...previous, [id]: next }
    })
  }, [])

  /** A instância acabou (fechamento aceito ou `session.ended`): solta tudo o que era dela. */
  const finishSession = useCallback(
    (id: string, how: 'closed' | 'ended') => {
      // O foco só sai do controle que vai sumir; quem já foi para outro lugar fica onde está.
      const button = closeButtonRef.current
      const active = document.activeElement
      if (
        button?.dataset.session === id &&
        (active === button || active === null || active === document.body)
      )
        mainRef.current?.focus()
      finished.current.add(id)
      releaseOpened(id)
      closeCommands.current.delete(id)
      unlockClose(id)
      setWarning((current) => (current === `close:${id}` ? null : current))
      setStatuses((previous) => {
        if (how === 'closed') return { ...previous, [id]: 'closed' }
        return id in previous ? without(previous, id) : previous
      })
    },
    [releaseOpened, unlockClose],
  )

  const receiveSession = useCallback(
    (event: EventEnvelopeV1) => {
      if (
        ![
          'urn:ads:event:session.created:1',
          'urn:ads:event:session.activity_changed:1',
          'urn:ads:event:session.ended:1',
        ].includes(event.type)
      )
        return
      if (seenEvents.current.has(event.id)) return
      seenEvents.current.add(event.id)

      const session = event as SessionEvent
      const id = session.data.session_id
      if (session.type === 'urn:ads:event:session.created:1' && finished.current.delete(id)) {
        // Uma nova instância com o mesmo id volta a aparecer.
        setStatuses((previous) => without(previous, id))
      }
      if (session.type === 'urn:ads:event:session.ended:1') finishSession(id, 'ended')

      setLiveEvents((previous) => [...previous, session])
    },
    [finishSession],
  )

  useEffect(
    () => () => {
      const opened = openedSubscriptions.current
      for (const entry of opened.values()) entry.stop()

      opened.clear()
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
    let current = true
    client.listHarnesses().then(
      (received) => {
        if (current) setCatalog({ generation, value: received })
      },
      (error: unknown) => {
        if (current)
          setCatalog({
            generation,
            value: error instanceof HarnessListError ? error.reason : 'refused',
          })
      },
    )
    return () => {
      current = false
    }
  }, [client, live, connection, generation])

  const liveCatalog =
    live && connection === 'connected' && catalog?.generation === generation ? catalog.value : null
  const listed = typeof liveCatalog === 'object' ? liveCatalog : null
  const available = live ? (listed?.harnesses ?? NO_STREAMS) : harnesses
  // Conectada, só abre com um apelido que o núcleo anunciou.
  const workdirAlias = !live
    ? workdir
    : listed?.workdir_aliases.includes(workdir)
      ? workdir
      : listed?.workdir_aliases[0]
  const catalogNotice: MessageKey | null =
    !live || connection !== 'connected'
      ? null
      : liveCatalog === null
        ? 'harness.loading'
        : typeof liveCatalog === 'string'
          ? FAILURE_NOTICE[liveCatalog]
          : liveCatalog.harnesses.length === 0
            ? 'harness.none'
            : workdirAlias === undefined
              ? 'harness.noWorkdir'
              : null

  useEffect(() => {
    if (!live || connection !== 'connected') return

    // Só esta assinatura terminou: as outras da mesma sessão continuam.
    const unsubscribe = sessionStreams.map((stream) =>
      client.subscribe(stream, 0, receiveSession, (code) => {
        if (stream.startsWith('session/'))
          markSession(stream.slice('session/'.length), statusAfter(code))
      }),
    )

    return () => {
      for (const stop of unsubscribe) stop()
    }
  }, [client, live, connection, sessionStreams, receiveSession, markSession])

  const { tasks, sessions } = useMemo(() => {
    if (live)
      return {
        tasks: [],
        sessions: withStatuses(sessionsFrom(liveEvents), statuses),
      }
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
    // A lista e o apelido são da conexão em que o botão apareceu.
    if (workdirAlias === undefined || client.getGeneration() !== generation) return
    setOpening(true)

    setWarning(null)

    const id = `s-${newUlid()}`

    // Assina antes da abertura: session.created pode preceder a resposta.

    setChosenSessionId(id)

    try {
      const entry: { stop: () => void } = { stop: () => undefined }
      entry.stop = client.subscribe(`session/${id}`, 0, receiveSession, (code) => {
        // Esta assinatura já terminou; esquece só ela, sem cancelar outra da mesma sessão.
        if (openedSubscriptions.current.get(id) === entry) releaseOpened(id)
        markSession(id, statusAfter(code))
      })

      openedSubscriptions.current.set(id, entry)

      const response = await client.send(
        newCommand('session.open', {
          session_id: id,
          harness,
          workdir_alias: workdirAlias,
          ...size.current,
        }),
      )

      // Depois de -32014 a assinatura continua: é por ela que chega o resultado do fechamento.
      if (response.error?.code === SESSION_SETUP_TIMED_OUT) markSession(id, 'setupFailed')
      else if (response.error) {
        releaseOpened(id)
        reportError()
      }
    } catch {
      releaseOpened(id)
      reportError()
    } finally {
      setOpening(false)
    }
  }

  const close = (id: string) => {
    if (closeLocks.current.has(id) || finished.current.has(id)) return

    // Depois de um prazo sem resposta o resultado é incerto: repete com a mesma chave e outro id.
    const previous = closeCommands.current.get(id)
    const command = previous
      ? { ...previous, id: newUlid() }
      : newCommand('session.close', { session_id: id })
    closeCommands.current.set(id, command)
    closeLocks.current.set(id, command)
    setClosingIds(new Set(closeLocks.current.keys()))
    setWarning(null)

    const fail = (definitive: boolean) => {
      // Outra tentativa da mesma chave ainda pode confirmar: só a última decide o aviso.
      if (finished.current.has(id) || closeLocks.current.get(id) !== command) return
      if (definitive) closeCommands.current.delete(id)
      unlockClose(id, command)
      setWarning(`close:${id}`)
    }

    // O prazo só devolve o botão; a resposta, quando vier, ainda conclui o fechamento.
    const timer = setTimeout(() => {
      fail(false)
    }, closeTimeoutMs)

    client.send(command).then(
      (response) => {
        clearTimeout(timer)
        if (finished.current.has(id)) return
        if (response.error) fail(true)
        else finishSession(id, 'closed')
      },
      () => {
        clearTimeout(timer)
        fail(false)
      },
    )
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

      {available.length > 0 && (
        <div className={styles.actions}>
          {available.map((harness) => (
            <button
              key={harness}
              type="button"
              className={styles.button}
              disabled={connection !== 'connected' || opening || workdirAlias === undefined}
              onClick={() => {
                void open(harness)
              }}
            >
              {t('session.open', { harness })}
            </button>
          ))}
        </div>
      )}

      {catalogNotice && (
        <p role={FAILURE_NOTICES.has(catalogNotice) ? 'alert' : 'status'}>{t(catalogNotice)}</p>
      )}

      {warning !== null && <p role="alert">{t('session.commandFailed')}</p>}

      {live && (
        <div className={styles.status} data-active={Boolean(session?.status)}>
          <p role="status" aria-label={t('session.statusLabel')}>
            {session?.status ? t(SESSION_NOTICE[session.status]) : ''}
          </p>
          {session?.status === 'closing' && (
            <button
              ref={closeButtonRef}
              type="button"
              className={styles.button}
              data-session={session.id}
              disabled={connection !== 'connected' || closingIds.has(session.id)}
              onClick={() => {
                close(session.id)
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
        <main ref={mainRef} id={TERMINAL_ID} className={styles.main} tabIndex={-1}>
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
