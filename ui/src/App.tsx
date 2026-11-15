import { useId, useMemo, useState } from 'react'
import { decodeBase64, useConnectionState, type CoreClient } from './api'
import styles from './App.module.css'
import { ConnectionStatus } from './components/ConnectionStatus'
import { Sidebar } from './components/Sidebar/Sidebar'
import { staticByteSource } from './components/Terminal/byteSource'
import { Terminal } from './components/Terminal/Terminal'
import { useT, type MessageKey } from './i18n'
import { sampleOutput, sampleSessions, sampleTasks } from './sample/data'
import type { ThemePreference } from './theme/theme'
import { useTheme } from './theme/useTheme'

const THEME_LABEL: Record<ThemePreference, MessageKey> = {
  system: 'theme.system',
  light: 'theme.light',
  dark: 'theme.dark',
}

const TERMINAL_ID = 'terminal'

/** Tela "em construção": a estrutura final (barra lateral, terminal, conexão) com dados de exemplo. */
export function App({ client }: { client: CoreClient }) {
  const t = useT()
  const sidebarId = useId()
  const connection = useConnectionState(client)
  const { preference, cycle } = useTheme()
  const [menuOpen, setMenuOpen] = useState(false)
  const [selectedSessionId, setSelectedSessionId] = useState(sampleSessions[0]?.id ?? '')

  const session = sampleSessions.find((item) => item.id === selectedSessionId)
  const sessionName = session ? t(session.nameKey) : ''
  const source = useMemo(
    () =>
      staticByteSource(decodeBase64(sampleOutput(t, selectedSessionId, sessionName).data.payload)),
    [t, selectedSessionId, sessionName],
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
      <p className={styles.notice}>{t('notice.wip')}</p>
      <div className={styles.body}>
        <Sidebar
          id={sidebarId}
          open={menuOpen}
          tasks={sampleTasks}
          sessions={sampleSessions}
          selectedSessionId={selectedSessionId}
          onSelectSession={(id) => {
            setSelectedSessionId(id)
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
