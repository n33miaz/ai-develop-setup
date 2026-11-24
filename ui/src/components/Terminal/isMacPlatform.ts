interface PlatformInfo {
  userAgentData?: { platform?: string | null } | null
  platform?: string | null
  userAgent?: string | null
}

export function isMacPlatform(nav?: PlatformInfo | null): boolean {
  const hint = nav?.userAgentData?.platform
  if (hint) return hint === 'macOS'
  if (nav?.platform) return nav.platform.startsWith('Mac')
  return nav?.userAgent?.includes('Macintosh') ?? false
}
