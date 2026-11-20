import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { selectCoreClient } from './api/bootstrap'
import { App } from './App'
import { createTranslator, I18nContext, resolveLocale } from './i18n'
import './styles/theme.css'
import './styles/global.css'

const root = document.getElementById('root')
if (!root) throw new Error('missing #root element')

const locale = resolveLocale(navigator.language)
document.documentElement.lang = locale

/** Inteiro não negativo da configuração, ou `undefined` para o padrão do cliente. */
function count(value: unknown): number | undefined {
  const parsed = Number(value)
  return value !== undefined && value !== '' && Number.isSafeInteger(parsed) && parsed >= 0
    ? parsed
    : undefined
}
const replayEvents = count(import.meta.env.VITE_CORE_REPLAY_EVENTS)
const replayBytes = count(import.meta.env.VITE_CORE_REPLAY_BYTES)
const { client, live } = selectCoreClient(window, undefined, {
  ...(replayEvents === undefined ? {} : { replayEvents }),
  ...(replayBytes === undefined ? {} : { replayBytes }),
})
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

createRoot(root).render(
  <StrictMode>
    <I18nContext value={createTranslator(locale)}>
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
