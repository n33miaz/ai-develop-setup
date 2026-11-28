import { spawn } from 'node:child_process'
import { setTimeout as delay } from 'node:timers/promises'
import { stripVTControlCharacters } from 'node:util'

const capturedErrorCharacters = 64 * 1024
const reportedErrorLines = 20
const reportedErrorCharacters = 4000
// Failure messages end up in public logs: no links and nothing as long as a run token.
const secrets = /[a-z][a-z0-9+.-]{0,30}:\/\/\S+|[\w-]{43,}/gi

function errorTail(captured, truncated) {
  // A truncated capture can start inside a link or a token, so its first line is dropped.
  const start = truncated ? captured.indexOf('\n') + 1 || captured.length : 0
  return stripVTControlCharacters(captured.slice(start))
    .replace(secrets, '[redacted]')
    .trim()
    .split(/\r?\n/)
    .slice(-reportedErrorLines)
    .join('\n')
    .slice(-reportedErrorCharacters)
}

// The browser drops a request that is in flight when its page navigates.
const droppedByNavigation = 'Inspected target navigated or closed'

export async function until(check, message, attempts = 150) {
  for (let attempt = 0; attempt < attempts; attempt++) {
    let result
    try {
      result = await check()
    } catch (error) {
      if (error.cause?.message !== droppedByNavigation) throw error
    }
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
    let errors = ''
    let truncated = false
    child.stderr.setEncoding('utf8').on('data', (chunk) => {
      errors += chunk
      if (errors.length <= capturedErrorCharacters) return
      errors = errors.slice(-capturedErrorCharacters)
      truncated = true
    })
    const errorsClosed = new Promise((done) => child.stderr.once('close', done))
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
        ).catch(async (error) => {
          // Output can arrive after the exit event, and a descendant can keep the pipe open.
          if (failure) await Promise.race([errorsClosed, delay(1000, undefined, { ref: false })])
          const tail = errorTail(errors, truncated)
          if (!tail) {
            checkProcess()
            throw error
          }
          const reason = failure ? `${message}: ${failure}` : error.message
          throw new Error(`${reason}\nLast lines of stderr:\n${tail}`)
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
      if (result.error) {
        throw new Error(`${method} failed: ${result.error.message}`, { cause: result.error })
      }
      return result.result
    } finally {
      clearTimeout(timer)
      pending.delete(id)
    }
  }
}
