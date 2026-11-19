import { expect, test } from 'vitest'
import config from '../../vite.config'

test('uses the same loopback address and strict port for development and preview', () => {
  const endpoint = { host: '127.0.0.1', port: 5173, strictPort: true }
  expect(config.server).toMatchObject(endpoint)
  expect(config.preview).toMatchObject(endpoint)
})
