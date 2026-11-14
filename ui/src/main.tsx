import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createFakeCoreClient } from './api'
import { App } from './App'
import { createTranslator, I18nContext, resolveLocale } from './i18n'
import './styles/theme.css'
import './styles/global.css'

const root = document.getElementById('root')
if (!root) throw new Error('missing #root element')

const locale = resolveLocale(navigator.language)
document.documentElement.lang = locale

// Sem núcleo ainda: o cliente falso fica desconectado e a tela mostra só dados de exemplo.
const client = createFakeCoreClient()

createRoot(root).render(
  <StrictMode>
    <I18nContext value={createTranslator(locale)}>
      <App client={client} />
    </I18nContext>
  </StrictMode>,
)
