import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import {
  desktopBridge,
  replaySettings,
  selectCoreClient,
  selectDesktopClient,
} from './api/bootstrap'
import { App } from './App'
import { createTranslator, I18nContext, resolveLocale } from './i18n'
import './styles/theme.css'
import './styles/global.css'

const root = document.getElementById('root')
if (!root) throw new Error('missing #root element')

const locale = resolveLocale(navigator.language)
document.documentElement.lang = locale

const configuredStreams: unknown = import.meta.env.VITE_CORE_SESSION_STREAMS
const sessionStreams =
  typeof configuredStreams === 'string'
    ? configuredStreams
        .split(',')
        .map((stream) => stream.trim())
        .filter(Boolean)
    : []

const harnesses = [
  ...new Set(
    String(import.meta.env.VITE_CORE_HARNESSES ?? '')
      .split(',')
      .map((value) => value.trim())
      .filter(Boolean),
  ),
]

const workdir = String(import.meta.env.VITE_CORE_WORKDIR ?? 'default')

const configuredDebounce = Number(import.meta.env.VITE_CORE_RESIZE_DEBOUNCE_MS ?? 150)

const resizeDebounceMs =
  Number.isFinite(configuredDebounce) && configuredDebounce >= 0 ? configuredDebounce : 150

function render(root: HTMLElement) {
  const replay = replaySettings(
    import.meta.env.VITE_CORE_REPLAY_EVENTS,
    import.meta.env.VITE_CORE_REPLAY_BYTES,
  )
  const bridge = desktopBridge(window)
  const t = createTranslator(locale)
  const react = createRoot(root)
  try {
    const { client, live } = bridge
      ? selectDesktopClient(
          bridge,
          replay,
          import.meta.env.VITE_DESKTOP_RETRY_INITIAL_MS,
          import.meta.env.VITE_DESKTOP_RETRY_MAX_MS,
          import.meta.env.VITE_DESKTOP_RETRY_STABLE_MS,
          import.meta.env.VITE_DESKTOP_RETRY_MAX_ATTEMPTS,
        )
      : selectCoreClient(window, undefined, replay)
    react.render(
      <StrictMode>
        <I18nContext value={t}>
          <App
            client={client}
            live={live}
            sessionStreams={sessionStreams}
            harnesses={harnesses}
            workdir={workdir}
            resizeDebounceMs={resizeDebounceMs}
          />
        </I18nContext>
      </StrictMode>,
    )
  } catch {
    react.render(<p role="alert">{t('connection.bootstrapFailed')}</p>)
  }
}

render(root)
