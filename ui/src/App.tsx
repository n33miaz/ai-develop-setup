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
  SESSION_SETUP_FAILED,
  useConnectionGeneration,
  useConnectionState,
  type CommandRequestV1,
  type CoreClient,
  type HarnessListFailure,
  type EventEnvelopeV1,
  type HarnessCatalog,
} from './api'

import { INVALID_PARAMS } from './api/contracts'
import { sessionInput } from './api/session'

import styles from './App.module.css'
import { ConnectionStatus } from './components/ConnectionStatus'
import { Sidebar } from './components/Sidebar/Sidebar'
import { queueApprovals } from './api/queueApprovals'
import { QueuePanel } from './components/QueuePanel'
import { ApprovalsPanel } from './components/ApprovalsPanel'
import { UsagePanel } from './components/UsagePanel/UsagePanel'
import { eventByteSource, staticByteSource } from './components/Terminal/byteSource'
import { Terminal } from './components/Terminal/Terminal'
import { useT, type MessageKey, type Translate } from './i18n'
import { SESSION_STREAM_PREFIX, SessionReplay } from './model/sessionReplay'
import {
  applySessionEvent,
  sessionsFrom,
  tasksFrom,
  type SessionStatus,
  type SessionView,
} from './model/views'
import { sampleEvents, sampleOutput, type SampleEvents } from './sample/data'
import { sampleUsage } from './sample/usage'
import type { ThemePreference } from './theme/theme'
import { useTheme } from './theme/useTheme'
import type { SessionEvent } from './api'
import { UpdateNotice } from './pwa/UpdateNotice'

function QueueAndApprovals({ api }: { api: ReturnType<typeof queueApprovals> }) {
  return (
    <>
      <QueuePanel api={api} />
      <ApprovalsPanel api={api} />
    </>
  )
}

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

type SessionStatuses = Readonly<Record<string, SessionStatus>>

interface OpenSubscription {
  stop: () => void
  // Até a resposta de `session.open`; com falha na preparação, até o resultado final da assinatura.
  preparing: boolean
}

function without(statuses: SessionStatuses, id: string): SessionStatuses {
  return Object.fromEntries(Object.entries(statuses).filter(([key]) => key !== id))
}

/** Junta o estado local às sessões dos eventos; fechamento pendente ou falha aparecem mesmo sem `session.created`. */
function withStatuses(sessions: SessionView[], statuses: SessionStatuses): SessionView[] {
  const views: SessionView[] = []
  for (const session of sessions) {
    const status = statuses[session.id]
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
  desktop?: boolean
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

export function App({
  client,
  events = sampleEvents,
  live = false,
  desktop = false,
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
  const controls = useMemo(() => queueApprovals(client), [client])
  const { preference, cycle } = useTheme()
  const compact = useSyncExternalStore(subscribeCompact, isCompact)
  const [menuOpen, setMenuOpen] = useState(false)

  // Quem levantou o aviso: o fechamento de uma sessão só apaga o aviso que ele mesmo levantou.
  const [warnings, setWarnings] = useState<ReadonlySet<string>>(new Set())

  const clearWarning = useCallback((key: string) => {
    setWarnings((current) => {
      if (!current.has(key)) return current
      const next = new Set(current)
      next.delete(key)
      return next
    })
  }, [])

  const reportError = useCallback(() => {
    setWarnings((current) => new Set(current).add('command'))
  }, [])

  // O aviso genérico vale até o próximo comando concluído com sucesso.
  const reportSuccess = useCallback(() => {
    clearWarning('command')
  }, [clearWarning])

  const mainRef = useRef<HTMLElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)

  // Assinaturas abertas por `open`, por sessão, até terminarem ou a sessão acabar.
  const openedSubscriptions = useRef(new Map<string, OpenSubscription>())

  const releaseOpened = useCallback((id: string) => {
    const entry = openedSubscriptions.current.get(id)
    openedSubscriptions.current.delete(id)
    entry?.stop()
  }, [])

  // A tentativa de fechamento em curso, por sessão (trava síncrona), e o comando cujo resultado
  // ainda é incerto: repetir usa a mesma chave de idempotência.
  const closeLocks = useRef(new Map<string, CommandRequestV1>())
  const closeCommands = useRef(new Map<string, CommandRequestV1>())

  // Um cursor por sessão visível ou fluxo configurado, inclusive após seu encerramento.
  const replay = useRef(new SessionReplay())

  const size = useRef({ cols: 80, rows: 24 })

  const [opening, setOpening] = useState(false)

  const [liveSessions, setLiveSessions] = useState<ReadonlyMap<string, SessionView>>(new Map())
  const [setupGenerations, setSetupGenerations] = useState<ReadonlyMap<string, number>>(new Map())

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
    if (replay.current.isFinished(id)) return
    setStatuses((previous) => {
      // Um resultado de outra assinatura não remove a ação de fechamento já disponível.
      if (previous[id] === 'closing' || previous[id] === status) return previous
      return { ...previous, [id]: status }
    })
  }, [])

  const finishSession = useCallback(
    (id: string, exited = false) => {
      const button = closeButtonRef.current
      const active = document.activeElement
      if (
        active?.closest<HTMLElement>('[data-session]')?.dataset.session === id ||
        (button?.dataset.session === id && (active === null || active === document.body))
      )
        mainRef.current?.focus()
      replay.current.finish(id, exited)
      releaseOpened(id)
      closeCommands.current.delete(id)
      unlockClose(id)
      clearWarning('close:' + id)
      setStatuses((previous) => (id in previous ? without(previous, id) : previous))
      setSetupGenerations((previous) => {
        if (!previous.has(id)) return previous
        const next = new Map(previous)
        next.delete(id)
        return next
      })
      setLiveSessions((previous) => {
        if (!previous.has(id)) return previous
        const next = new Map(previous)
        next.delete(id)
        return next
      })
    },
    [clearWarning, releaseOpened, unlockClose],
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

      const session = event as SessionEvent
      const id = session.data.session_id
      const wasFinished = replay.current.isFinished(id)
      if (!replay.current.receive(session)) return
      if (session.type === 'urn:ads:event:session.ended:1') {
        finishSession(id, true)
        return
      }
      if (wasFinished) setStatuses((previous) => without(previous, id))
      setLiveSessions((previous) => {
        const next = new Map(previous)
        applySessionEvent(next, session)
        return next
      })
    },
    [finishSession],
  )

  useEffect(
    () => () => {
      const opened = openedSubscriptions.current
      for (const entry of opened.values()) entry.stop()

      opened.clear()
    },
    // A geração refaz tudo numa reconexão que a tela não chegou a ver.
    [client, connection, generation],
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
        // Erro de uma conexão que já foi trocada não diz nada sobre a atual.
        if (current && client.getGeneration() === generation)
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
  const unavailableWorkdirs = listed?.missing_workdir_aliases
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
            : workdirAlias === undefined && !unavailableWorkdirs
              ? 'harness.noWorkdir'
              : null

  useEffect(() => {
    replay.current.configure(sessionStreams)
    if (!live || connection !== 'connected') return

    // A resposta final pertence à instância vista por esta assinatura.
    const unsubscribe = sessionStreams.map((stream) => {
      const id = stream.slice(SESSION_STREAM_PREFIX.length)
      let creation: string | undefined
      return client.subscribe(
        stream,
        0,
        (event) => {
          if (event.type === 'urn:ads:event:session.created:1') creation = event.id
          receiveSession(event)
        },
        (code) => {
          if (!stream.startsWith(SESSION_STREAM_PREFIX)) return
          if (creation !== undefined && creation !== replay.current.creationOf(id)) return
          if (code === SESSION_CLOSE_PENDING && creation !== undefined) {
            replay.current.resumeCleanup(id)
          }
          markSession(id, code === SESSION_CLOSE_PENDING ? 'closing' : 'ended')
        },
      )
    })
    return () => {
      for (const stop of unsubscribe) stop()
    }
  }, [client, live, connection, generation, sessionStreams, receiveSession, markSession])

  const { tasks, sessions } = useMemo(() => {
    if (live)
      return {
        tasks: [],
        sessions: withStatuses([...liveSessions.values()], statuses),
      }
    const replayed = events(t)
    return { tasks: tasksFrom(replayed.tasks), sessions: sessionsFrom(replayed.sessions) }
  }, [events, t, live, liveSessions, statuses])
  const [chosenSessionId, setChosenSessionId] = useState<string | null>(null)

  // Seleção sempre válida: a escolhida enquanto existir, senão a primeira, senão nenhuma.
  const session = sessions.find((item) => item.id === chosenSessionId) ?? sessions[0]
  const sessionId = session?.id
  const sessionName = session ? (session.displayName ?? t('session.unnamed')) : ''
  // Fonte e entrada são da conexão autenticada atual (0: nenhuma); uma reconexão as refaz.
  const liveGeneration = live && connection === 'connected' ? generation : 0
  const source = useMemo(
    () =>
      liveGeneration > 0 && sessionId !== undefined
        ? eventByteSource(client, `session/${sessionId}`)
        : staticByteSource(
            live
              ? ''
              : sessionId === undefined
                ? ''
                : decodeBase64(sampleOutput(t, sessionId, sessionName).data.payload),
          ),
    [t, sessionId, sessionName, live, client, liveGeneration],
  )

  const input = useMemo(
    () =>
      liveGeneration > 0 && sessionId
        ? sessionInput(client, sessionId, reportError, reportSuccess)
        : undefined,

    [liveGeneration, sessionId, client, reportError, reportSuccess],
  )

  const resize = useCallback(
    (cols: number, rows: number) => {
      size.current = { cols: Math.min(1000, cols), rows: Math.min(1000, rows) }

      if (!input || !sessionId) return

      void client
        .send(newCommand('session.resize', { session_id: sessionId, ...size.current }))

        .then((response) => {
          if (response.error) reportError()
          else reportSuccess()
        }, reportError)
    },
    [client, input, sessionId, reportError, reportSuccess],
  )

  const open = async (harness: string) => {
    // A lista e o apelido são da conexão em que o botão apareceu.
    if (workdirAlias === undefined || client.getGeneration() !== generation) return
    setOpening(true)

    clearWarning('command')

    const id = `s-${newUlid()}`

    // Assina antes da abertura para receber o resultado final de uma preparação que falhe. Nesta
    // conexão os eventos da sessão só chegam depois da resposta; numa falha, nenhum chega.

    setChosenSessionId(id)

    try {
      const entry: OpenSubscription = { stop: () => undefined, preparing: true }
      openedSubscriptions.current.set(id, entry)
      entry.stop = client.subscribe(SESSION_STREAM_PREFIX + id, 0, receiveSession, (code) => {
        if (openedSubscriptions.current.get(id) !== entry) return
        releaseOpened(id)
        if (code === SESSION_CLOSE_PENDING) {
          markSession(id, 'closing')
        } else if (entry.preparing) {
          finishSession(id)
        } else {
          markSession(id, 'ended')
        }
      })

      const response = await client.send(
        newCommand('session.open', {
          session_id: id,
          harness,
          workdir_alias: workdirAlias,
          ...size.current,
        }),
      )

      if (openedSubscriptions.current.get(id) !== entry) return
      if (response.error) {
        // O terminal chegou a abrir: a assinatura fica para o resultado do fechamento automático.
        if (response.error.code === SESSION_SETUP_FAILED) {
          setSetupGenerations((previous) => new Map(previous).set(id, generation))
          markSession(id, 'setupFailed')
        } else {
          releaseOpened(id)
          reportError()
        }
      } else {
        entry.preparing = false
        reportSuccess()
      }
    } catch {
      releaseOpened(id)
      reportError()
    } finally {
      setOpening(false)
    }
  }

  const close = (id: string) => {
    if (closeLocks.current.has(id) || replay.current.isFinished(id)) return

    // Depois de um prazo sem resposta o resultado é incerto: repete com a mesma chave e outro id.
    const previous = closeCommands.current.get(id)
    const command = previous
      ? { ...previous, id: newUlid() }
      : newCommand('session.close', { session_id: id })
    closeCommands.current.set(id, command)
    closeLocks.current.set(id, command)
    setClosingIds(new Set(closeLocks.current.keys()))
    clearWarning('close:' + id)

    const fail = (definitive: boolean) => {
      // Outra tentativa da mesma chave ainda pode confirmar: só a última decide o aviso.
      if (closeLocks.current.get(id) !== command) return
      if (definitive) closeCommands.current.delete(id)
      unlockClose(id, command)
      setWarnings((current) => new Set(current).add('close:' + id))
    }

    // O prazo só devolve o botão; a resposta, quando vier, ainda conclui o fechamento.
    const timer = setTimeout(() => {
      fail(false)
    }, closeTimeoutMs)

    client.send(command).then(
      (response) => {
        clearTimeout(timer)
        if (response.error && response.error.code !== INVALID_PARAMS) {
          fail(true)
          return
        }
        if (!response.error) reportSuccess()
        if (
          closeCommands.current.get(id)?.params._meta.idempotency_key ===
          command.params._meta.idempotency_key
        )
          finishSession(id)
      },
      () => {
        clearTimeout(timer)
        fail(false)
      },
    )
  }

  const warningMessages = sessions
    .filter(
      ({ id, status }) =>
        (status === 'closing' || status === 'setupFailed') && warnings.has('close:' + id),
    )
    .map(({ displayName }) =>
      t('session.closeFailed', { name: displayName ?? t('session.unnamed') }),
    )
  if (warnings.has('command')) warningMessages.unshift(t('session.commandFailed'))

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
      {unavailableWorkdirs && (
        <p role="alert">
          {t('harness.unavailableWorkdirs', { aliases: unavailableWorkdirs.join(', ') })}
        </p>
      )}

      {warningMessages.length > 0 && <p role="alert">{warningMessages.join(' ')}</p>}

      {live && (
        <div className={styles.status} data-active={Boolean(session?.status)}>
          <p role="status" aria-label={t('session.statusLabel')}>
            {session?.status ? t(SESSION_NOTICE[session.status]) : ''}
          </p>
          {(session?.status === 'closing' ||
            (session?.status === 'setupFailed' &&
              generation !== setupGenerations.get(session.id))) && (
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

      {!live && <p className={styles.notice}>{t('notice.sample')}</p>}
      {live && connection === 'connected' && <p className={styles.notice}>{t('notice.live')}</p>}
      <UpdateNotice className={styles.notice} />
      <UsagePanel tools={live ? null : sampleUsage} />
      {live && connection === 'connected' && <QueueAndApprovals key={generation} api={controls} />}
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
            browserClipboard={!desktop}
          />
        </main>
      </div>
    </div>
  )
}
