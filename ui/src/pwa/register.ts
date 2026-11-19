export async function registerPwa(
  production: boolean,
  secure: boolean,
  serviceWorker: ServiceWorkerContainer | undefined,
  base: string,
  onUpdate: () => void,
): Promise<void> {
  if (!production || !secure || !serviceWorker) return

  const previous = await serviceWorker.getRegistration(base)
  previous?.waiting?.postMessage('activate-update')

  const hadController = Boolean(serviceWorker.controller)
  const registration = await serviceWorker.register(new URL('service-worker.js', base), {
    type: 'module',
  })

  const observeInstallation = () => {
    const worker = registration.installing
    if (!hadController || !worker) return
    const notifyIfReady = () => {
      if (worker.state === 'installed') onUpdate()
    }
    worker.addEventListener('statechange', notifyIfReady)
    notifyIfReady()
  }
  registration.addEventListener('updatefound', observeInstallation)
  observeInstallation()
}
