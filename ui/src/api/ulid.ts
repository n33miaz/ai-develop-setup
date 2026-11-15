const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'
const TIME_CHARS = 10
const RANDOM_CHARS = 16

/** ULID novo (26 caracteres em Crockford base32): 48 bits de tempo e 80 de acaso. */
export function newUlid(now: number = Date.now()): string {
  let time = ''
  let rest = now
  for (let i = 0; i < TIME_CHARS; i += 1) {
    time = CROCKFORD.charAt(rest % 32) + time
    rest = Math.floor(rest / 32)
  }
  let random = ''
  for (const byte of crypto.getRandomValues(new Uint8Array(RANDOM_CHARS))) {
    random += CROCKFORD.charAt(byte % 32)
  }
  return time + random
}
