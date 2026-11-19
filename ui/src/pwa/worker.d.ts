export function staticResponse(
  request: Pick<Request, 'url' | 'method' | 'mode'>,
  allowed: ReadonlySet<string>,
  caches: Pick<CacheStorage, 'match'>,
  cacheName: string,
): Promise<Response> | undefined
