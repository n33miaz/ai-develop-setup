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

const { client, live } = selectCoreClient(window)
const configuredStreams: unknown = import.meta.env.VITE_CORE_SESSION_STREAMS
const sessionStreams =
  typeof configuredStreams === 'string'
    ? configuredStreams
        .split(',')
        .map((stream) => stream.trim())
        .filter(Boolean)
    : []

createRoot(root).render(
  <StrictMode>
    <I18nContext value={createTranslator(locale)}>
      <App client={client} live={live} sessionStreams={sessionStreams} />
    </I18nContext>
  </StrictMode>,
)
