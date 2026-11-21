import { useEffect, useState } from 'react'
import { desktopBridge } from '../api/bootstrap'
import { useT } from '../i18n'
import { registerPwa } from './register'

export function UpdateNotice({ className }: { className: string | undefined }) {
  const t = useT()
  const [available, setAvailable] = useState(false)

  useEffect(() => {
    if (desktopBridge(window)) return
    let active = true
    const base = new URL(import.meta.env.BASE_URL, document.baseURI).href
    void registerPwa(
      import.meta.env.PROD,
      window.isSecureContext,
      navigator.serviceWorker,
      base,
      () => {
        if (active) setAvailable(true)
      },
    ).catch(() => undefined)
    return () => {
      active = false
    }
  }, [])

  return available ? (
    <p className={className} role="status">
      {t('pwa.update')}
    </p>
  ) : null
}
