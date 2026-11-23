import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'

export async function withTestProcesses(run) {
  const children = []
  function start(program, args, options = {}) {
    const child = spawn(program, args, { stdio: ['ignore', 'pipe', 'pipe'], ...options })
    children.push(child)
    let output = ''
    child.stdout.on('data', (chunk) => (output += chunk))
    child.stderr.resume()
    child.on('error', () => {})
    return { child, output: () => output }
  }
  try {
    return await run(start)
  } finally {
    for (const child of children.reverse()) await stop(child)
  }
}

export async function stop(child) {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return
  await new Promise((done) => {
    child.once('exit', done)
    child.kill()
  })
}

export function cdpRequests(ws, timeoutMs = 15000) {
  let nextId = 0
  let disconnected = false
  const pending = new Map()
  function disconnect() {
    disconnected = true
    for (const request of pending.values()) request.reject(new Error('Browser disconnected'))
    pending.clear()
  }
  ws.addEventListener('close', disconnect, { once: true })
  ws.addEventListener('error', disconnect, { once: true })
  ws.addEventListener('message', ({ data }) => {
    const result = JSON.parse(data)
    pending.get(result.id)?.resolve(result)
  })
  return async function send(method, params) {
    if (disconnected) throw new Error('Browser disconnected')
    const id = ++nextId
    let timer
    try {
      const result = await new Promise((resolve, reject) => {
        timer = setTimeout(() => reject(new Error(`${method} timed out`)), timeoutMs)
        pending.set(id, { resolve, reject })
        ws.send(JSON.stringify({ id, method, params }))
      })
      assert.ok(!result.error, `${method} failed`)
      return result.result
    } finally {
      clearTimeout(timer)
      pending.delete(id)
    }
  }
}
