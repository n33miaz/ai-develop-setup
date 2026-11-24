import { describe, expect, it } from 'vitest'
import { isMacPlatform } from './isMacPlatform'

describe('isMacPlatform', () => {
  const macUserAgent = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)'

  it('prefers the client hint over the legacy platform', () => {
    expect(isMacPlatform({ userAgentData: { platform: 'macOS' }, platform: 'Win32' })).toBe(true)
    expect(isMacPlatform({ userAgentData: { platform: 'Windows' }, platform: 'MacIntel' })).toBe(
      false,
    )
  })

  it('uses the legacy platform when the client hint is empty or absent', () => {
    expect(isMacPlatform({ userAgentData: { platform: '' }, platform: 'MacIntel' })).toBe(true)
    expect(isMacPlatform({ platform: 'Linux x86_64', userAgent: macUserAgent })).toBe(false)
  })

  it('uses the user agent when both platform fields are unavailable', () => {
    expect(isMacPlatform({ platform: '', userAgent: macUserAgent })).toBe(true)
    expect(isMacPlatform({ userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' })).toBe(false)
    expect(isMacPlatform({ userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS like Mac OS X)' })).toBe(
      false,
    )
  })

  it.each([
    undefined,
    null,
    {},
    { userAgentData: null },
    { userAgentData: {} },
    { userAgentData: { platform: null }, platform: null, userAgent: null },
    { userAgentData: { platform: '' }, platform: '', userAgent: '' },
  ])('handles missing or empty navigator data (%j)', (nav) => {
    expect(isMacPlatform(nav)).toBe(false)
  })
})
