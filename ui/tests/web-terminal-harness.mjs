import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { setTimeout as delay } from 'node:timers/promises'
import { stripVTControlCharacters } from 'node:util'

export async function until(check, message, attempts = 150) {
  for (let attempt = 0; attempt < attempts; attempt++) {
    const result = await check()
    if (result) return result
    await delay(100)
  }
  throw new Error(message)
}

export async function withTestProcesses(run) {
  const children = []
  function start(program, args, options = {}) {
    let child
    try {
      child = spawn(program, args, { stdio: ['ignore', 'pipe', 'pipe'], ...options })
    } catch {
      throw new Error('Could not start test process')
    }
    children.push(child)
    let output = ''
    child.stdout.on('data', (chunk) => (output += chunk))
    child.stderr.resume()
    let failure
    child.on('error', () => {
      failure ??= 'could not start process'
    })
    child.once('exit', (code, signal) => {
      failure ??= signal ? `process exited on ${signal}` : `process exited with code ${code}`
    })
    return {
      child,
      waitForReady(check, message, attempts) {
        const checkProcess = () => {
          if (failure) throw new Error(`${message}: ${failure}`)
        }
        return until(
          async () => {
            checkProcess()
            // Vite colors the port in its URL under CI and on Windows.
            const result = await check(stripVTControlCharacters(output))
            checkProcess()
            return result
          },
          message,
          attempts,
        ).catch((error) => {
          checkProcess()
          throw error
        })
      },
    }
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
