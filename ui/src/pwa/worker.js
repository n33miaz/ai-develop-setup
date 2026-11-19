export function staticResponse(request, allowed, caches, cacheName) {
  const url = new URL(request.url)
  if (
    request.method !== 'GET' ||
    request.mode === 'navigate' ||
    !['http:', 'https:'].includes(url.protocol) ||
    !allowed.has(url.href)
  ) {
    return undefined
  }
  return caches.match(url.href, { cacheName }).then((response) => response ?? fetch(request))
}

if (typeof ServiceWorkerGlobalScope !== 'undefined' && self instanceof ServiceWorkerGlobalScope) {
  const scope = self.registration.scope
  const prefix = 'ads-static:' + encodeURIComponent(scope) + ':'
  const cacheName = prefix + version
  const allowed = new Set(assets.map((asset) => new URL(asset, scope).href))

  self.addEventListener('install', (event) => {
    const requests = [...allowed].map(
      (url) => new Request(url, { cache: 'reload', credentials: 'omit', redirect: 'error' }),
    )
    event.waitUntil(caches.open(cacheName).then((cache) => cache.addAll(requests)))
  })

  self.addEventListener('activate', (event) => {
    event.waitUntil(
      caches
        .keys()
        .then((names) =>
          Promise.all(
            names
              .filter((name) => name.startsWith(prefix) && name !== cacheName)
              .map((name) => caches.delete(name)),
          ),
        ),
    )
  })

  self.addEventListener('message', (event) => {
    if (event.data === 'activate-update') event.waitUntil(self.skipWaiting())
  })

  self.addEventListener('fetch', (event) => {
    const response = staticResponse(event.request, allowed, caches, cacheName)
    if (response) event.respondWith(response)
  })
}
