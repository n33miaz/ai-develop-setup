import assert from 'node:assert/strict'
import { once } from 'node:events'
import { lstat, mkdir, mkdtemp, readdir, rmdir, unlink, writeFile } from 'node:fs/promises'
import { createServer } from 'node:net'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { cdpRequests, stop, until, withTestProcesses } from './web-terminal-harness.mjs'
import { browserEnvironment, withTestDirectory } from './web-terminal-scratch.mjs'

test('partial startup preserves the failure and reaps every started process', async (t) => {
  const started = []
  t.after(async () => {
    for (const child of started) await stop(child)
  })
  await assert.rejects(
    withTestProcesses(async (start) => {
      for (let i = 0; i < 2; i++) {
        const { child } = start(process.execPath, ['-e', 'setTimeout(() => {}, 30000)'], {
          env: {},
        })
        started.push(child)
        await once(child, 'spawn')
      }
      throw new Error('partial startup')
    }),
    /^Error: partial startup$/,
  )
  for (const child of started) assert.ok(child.exitCode !== null || child.signalCode !== null)
})

test('readiness reports a spawn failure without exposing the command', async () => {
  await assert.rejects(
    withTestProcesses(async (start) => {
      const process = start('https://example.invalid/#private-link', [])
      await process.waitForReady(() => false, 'Terminal host did not become ready', 2)
    }),
    { message: 'Terminal host did not become ready: could not start process' },
  )
})

test('readiness reports an early exit without exposing standard output', async () => {
  await assert.rejects(
    withTestProcesses(async (start) => {
      const daemon = start(process.execPath, [
        '-e',
        `console.log('https://example.invalid/#private-link'); process.exitCode = 23`,
      ])
      await daemon.waitForReady(() => false, 'Daemon did not become ready', 50)
    }),
    { message: 'Daemon did not become ready: process exited with code 23' },
  )
})

async function startupFailure(script, attempts = 50) {
  let failure
  await assert.rejects(
    withTestProcesses(async (start) => {
      const child = start(process.execPath, ['-e', script])
      await child.waitForReady(() => false, 'Process did not become ready', attempts)
    }),
    (error) => {
      failure = error.message
      return true
    },
  )
  return failure
}

test('a startup failure reports the last lines of the error output', async () => {
  const message = await startupFailure(String.raw`
    for (let line = 0; line < 100; line++) console.error('noise ' + line + '.')
    console.error('\x1b[31mfatal: socket path too long\x1b[0m')
    process.exitCode = 23
  `)
  const lines = message.split('\n')
  assert.equal(lines[0], 'Process did not become ready: process exited with code 23')
  assert.equal(lines[1], 'Last lines of stderr:')
  assert.equal(lines.at(-1), 'fatal: socket path too long')
  assert.equal(lines.length, 22)
  assert.ok(!message.includes('noise 80.'))
})

test('a startup timeout reports the error output of a running process', async () => {
  const message = await startupFailure(
    `console.error('still waiting'); setTimeout(() => {}, 30000)`,
    20,
  )
  assert.equal(message, 'Process did not become ready\nLast lines of stderr:\nstill waiting')
})

test('the reported error output has a size limit', async () => {
  const message = await startupFailure(
    `process.stderr.write('long line '.repeat(5000) + 'end'); process.exitCode = 1`,
  )
  assert.ok(message.endsWith('end'))
  assert.ok(message.length < 4200)
})

test('the reported error output hides access links and tokens', async () => {
  const token = 'Ab0_-'.repeat(9)
  const message = await startupFailure(`
    console.error('open http://127.0.0.1:5173/#core=ws://127.0.0.1:1&token=${token} now')
    console.error('listening on ws://127.0.0.1:1234/devtools')
    console.error('bootstrap 1234 ${token}')
    process.exitCode = 1
  `)
  assert.deepEqual(message.split('\n').slice(2), [
    'open [redacted] now',
    'listening on [redacted]',
    'bootstrap 1234 [redacted]',
  ])
})

test('a token cut by the capture limit is not reported', async () => {
  const token = 'Ab0_-'.repeat(9)
  // The last 64 KiB of this output start inside the token.
  const message = await startupFailure(`
    process.stderr.write('x '.repeat(10) + '${token}' + ' http://h/' + 'a'.repeat(65510) + '\\nlast\\n')
    process.exitCode = 1
  `)
  assert.deepEqual(message.split('\n').slice(2), ['last'])
})

test('readiness matches output that a process colors', async () => {
  await withTestProcesses(async (start) => {
    const server = start(process.execPath, [
      '-e',
      String.raw`console.log('http://127.0.0.1:\x1b[1m5173\x1b[22m/'); setTimeout(() => {}, 30000)`,
    ])
    await server.waitForReady(
      (output) => output.includes('http://127.0.0.1:5173'),
      'Server did not become ready',
      50,
    )
  })
})

const root = fileURLToPath(new URL('../..', import.meta.url))

test(
  'a long checkout path leaves room for the Unix socket',
  { skip: process.platform === 'win32' },
  async (t) => {
    const parent = join(root, '.test-tmp')
    await mkdir(parent, { recursive: true })
    const longRoot = await mkdtemp(join(parent, 'long-checkout-'.repeat(9)))
    t.after(async () => {
      await rmdir(join(longRoot, '.test-tmp'))
      await rmdir(longRoot)
    })

    const server = createServer()
    let socketPath
    try {
      await withTestDirectory(
        longRoot,
        async ({ scratch, channelName, socketDirectory = scratch }) => {
          socketPath = join(socketDirectory, `${channelName}.sock`)
          assert.ok(Buffer.byteLength(join(scratch, `${channelName}.sock`)) > 107)
          assert.ok(Buffer.byteLength(socketPath) <= 103, 'Unix socket path exceeds 103 bytes')
          server.listen(socketPath)
          await once(server, 'listening')
          assert.ok((await lstat(socketPath)).isSocket())
        },
      )
      await assert.rejects(readdir(dirname(socketPath)), { code: 'ENOENT' })
    } finally {
      if (server.listening) await new Promise((resolve) => server.close(resolve))
    }
  },
)

test(
  'a long checkout path leaves room for the browser socket',
  { skip: process.platform === 'win32' },
  async (t) => {
    const parent = join(root, '.test-tmp')
    await mkdir(parent, { recursive: true })
    const longRoot = await mkdtemp(join(parent, 'long-checkout-'.repeat(9)))
    t.after(async () => {
      await rmdir(join(longRoot, '.test-tmp'))
      await rmdir(longRoot)
    })

    await withTestDirectory(longRoot, async (paths) => {
      const { TMPDIR } = browserEnvironment({ TMPDIR: paths.scratch }, paths)
      // Chrome creates this directory and socket below TMPDIR and aborts when they do not fit.
      const directory = join(TMPDIR, 'com.google.Chrome.XXXXXX')
      const socketPath = join(directory, 'SingletonSocket')
      assert.ok(
        Buffer.byteLength(socketPath) <= 107,
        `Browser socket path exceeds 107 bytes: ${socketPath}`,
      )
      await mkdir(directory)
      const server = createServer()
      try {
        server.listen(socketPath)
        await once(server, 'listening')
        assert.ok((await lstat(socketPath)).isSocket())
      } finally {
        if (server.listening) await new Promise((resolve) => server.close(resolve))
        await rmdir(directory)
      }
    })
  },
)

test('the browser keeps its scratch directory when no short one exists', () => {
  assert.deepEqual(browserEnvironment({ PATH: 'bin' }, { scratch: 'scratch' }), {
    PATH: 'bin',
    TMPDIR: 'scratch',
  })
})

test('browser scratch removes known files and empty directories on success', async (t) => {
  let scratch
  await withTestDirectory(root, async (paths) => {
    scratch = paths.scratch
    t.after(async () => {
      for (const file of [paths.hostConfig, paths.daemonConfig, paths.devToolsPort]) {
        await ignoreMissing(unlink(file))
      }
      for (const directory of [paths.profile, paths.workdir, scratch]) {
        await ignoreMissing(rmdir(directory))
      }
    })
    for (const file of [paths.hostConfig, paths.daemonConfig, paths.devToolsPort]) {
      await writeFile(file, 'fixture')
    }
  })
  await assert.rejects(readdir(scratch), { code: 'ENOENT' })
})

test('browser scratch preserves unexpected entries and the original failure', async (t) => {
  const reports = []
  const failure = new Error('browser failed')
  let paths
  await assert.rejects(
    withTestDirectory(
      root,
      async (layout) => {
        paths = layout
        const extra = join(paths.profile, 'unexpected')
        t.after(async () => {
          for (const file of [paths.hostConfig, paths.devToolsPort, extra]) {
            await ignoreMissing(unlink(file))
          }
          for (const directory of [
            paths.daemonConfig,
            paths.profile,
            paths.workdir,
            paths.scratch,
          ]) {
            await ignoreMissing(rmdir(directory))
          }
        })
        await writeFile(paths.hostConfig, 'fixture')
        await writeFile(paths.devToolsPort, 'fixture')
        await writeFile(extra, 'preserve')
        await mkdir(paths.daemonConfig)
        throw failure
      },
      (message) => reports.push(message),
    ),
    (error) => error === failure,
  )
  assert.deepEqual((await readdir(paths.scratch)).sort(), ['browser', 'daemon.toml'])
  assert.deepEqual(await readdir(paths.profile), ['unexpected'])
  assert.equal(reports.length, 1)
  assert.ok(reports[0].includes(paths.scratch))
  assert.match(reports[0], /browser \(1 retained\).*root \(1 retained\)/)
})

async function ignoreMissing(operation) {
  try {
    await operation
  } catch (error) {
    if (error.code !== 'ENOENT') throw error
  }
}

class TestSocket extends EventTarget {
  send(message) {
    this.lastId = JSON.parse(message).id
  }

  reply(id, result, error) {
    const data = JSON.stringify({ id, result, error })
    this.dispatchEvent(new MessageEvent('message', { data }))
  }
}

test('polling asks again when a navigation drops an evaluation', { timeout: 2000 }, async () => {
  const ws = new TestSocket()
  const send = cdpRequests(ws)
  const dropped = { code: -32000, message: 'Inspected target navigated or closed' }
  let polls = 0
  const result = await until(
    () => {
      const request = send('Runtime.evaluate')
      if (++polls === 1) ws.reply(ws.lastId, undefined, dropped)
      else ws.reply(ws.lastId, 'loaded')
      return request
    },
    'Page did not load',
    3,
  )
  assert.equal(result, 'loaded')
  assert.equal(polls, 2)
})

test('polling stops on any other protocol error', { timeout: 2000 }, async () => {
  const ws = new TestSocket()
  const send = cdpRequests(ws)
  let polls = 0
  await assert.rejects(
    until(
      () => {
        polls++
        const request = send('Runtime.evaluate')
        ws.reply(ws.lastId, undefined, { code: -32602, message: 'Invalid parameters' })
        return request
      },
      'Page did not load',
      3,
    ),
    { message: 'Runtime.evaluate failed: Invalid parameters' },
  )
  assert.equal(polls, 1)
})

for (const event of ['close', 'error']) {
  test(`all CDP calls reject on socket ${event}`, { timeout: 2000 }, async () => {
    const ws = new TestSocket()
    const send = cdpRequests(ws)
    const calls = [send('Page.navigate'), send('Runtime.evaluate')]
    ws.dispatchEvent(new Event(event))
    calls.push(send('Page.bringToFront'))
    await Promise.all(calls.map((call) => assert.rejects(call, /disconnected/)))
  })
}

test('a late CDP response cannot complete another request', { timeout: 2000 }, async () => {
  const ws = new TestSocket()
  const send = cdpRequests(ws, 50)
  const first = send('Page.navigate')
  const firstId = ws.lastId
  await assert.rejects(first, /Page.navigate timed out/)
  const second = send('Runtime.evaluate')
  ws.reply(firstId, 'late')
  ws.reply(ws.lastId, 'current')
  assert.equal(await second, 'current')
})
