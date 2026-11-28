import assert from 'node:assert/strict'
import { once } from 'node:events'
import fs from 'node:fs/promises'
import { lstat, mkdir, mkdtemp, readdir, rmdir, unlink, writeFile } from 'node:fs/promises'
import { syncBuiltinESMExports } from 'node:module'
import { createServer } from 'node:net'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { cdpRequests, stop, withTestProcesses } from './web-terminal-harness.mjs'
import { withTestDirectory } from './web-terminal-scratch.mjs'

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

test('readiness reports an early exit without exposing process output', async () => {
  await assert.rejects(
    withTestProcesses(async (start) => {
      const daemon = start(process.execPath, [
        '-e',
        `console.log('https://example.invalid/#private-link'); console.error('private error'); process.exitCode = 23`,
      ])
      await daemon.waitForReady(() => false, 'Daemon did not become ready', 10)
    }),
    { message: 'Daemon did not become ready: process exited with code 23' },
  )
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
  assert.equal(reports.length, 2)
  assert.ok(reports[0].includes(paths.scratch))
  assert.match(reports[0], /browser \(1 retained\).*root \(1 retained\)/)
})

test('browser scratch reports the cleanup error code without replacing the test failure', async (t) => {
  const fixture = await createScratchFixture()
  const reports = []
  const failure = new Error('browser failed')
  const privatePath = '/third-party/private-profile'
  const blocked = Object.assign(new Error(`unlink failed: ${privatePath}`), {
    code: 'EBUSY',
    path: privatePath,
  })
  let paths
  const mock = t.mock.method(fs, 'unlink', async (path) => {
    if (path === paths?.hostConfig) throw blocked
    return unlinkFile(path)
  })
  syncBuiltinESMExports()
  t.after(async () => {
    mock.mock.restore()
    syncBuiltinESMExports()
    if (paths) {
      await ignoreMissing(unlink(paths.hostConfig))
      await ignoreMissing(unlink(paths.daemonConfig))
      await ignoreMissing(rmdir(paths.profile))
      await ignoreMissing(rmdir(paths.workdir))
      await ignoreMissing(rmdir(paths.scratch))
    }
    await removeScratchFixture(fixture)
  })
  await assert.rejects(
    withTestDirectory(
      fixture,
      async (layout) => {
        paths = layout
        await writeFile(paths.hostConfig, 'blocked')
        await writeFile(paths.daemonConfig, 'removable')
        throw failure
      },
      (message) => reports.push(message),
    ),
    (error) => error === failure,
  )
  assert.deepEqual(await readdir(paths.scratch), ['host.toml'])
  assert.match(reports.join('\n'), /cleanup incomplete \(EBUSY\)/)
  assert.ok(!reports.join('\n').includes(privatePath))
})

const unlinkFile = unlink

test('browser scratch totals retained directories and file bytes without following links', async (t) => {
  const fixture = await createScratchFixture()
  const parent = join(fixture, '.test-tmp')
  const old = join(parent, 'web-terminal-previous')
  const other = join(parent, 'unrelated')
  const oldFile = join(old, 'payload')
  const otherFile = join(other, 'payload')
  const reports = []
  await mkdir(old)
  await mkdir(other)
  await writeFile(oldFile, '12345')
  await writeFile(otherFile, 'ignore these bytes')
  t.after(async () => {
    await unlink(oldFile)
    await rmdir(old)
    await unlink(otherFile)
    await rmdir(other)
  })
  const linkedScratch = join(parent, 'web-terminal-link')
  await withTestDirectory(
    fixture,
    async ({ profile, scratch }) => {
      const retained = join(profile, 'retained')
      const retainedFile = join(retained, 'payload')
      const linkedDirectory = join(retained, 'linked-directory')
      t.after(async () => {
        await ignoreMissing(unlink(linkedScratch))
        await ignoreMissing(unlink(linkedDirectory))
        await ignoreMissing(unlink(retainedFile))
        await ignoreMissing(rmdir(retained))
        await ignoreMissing(rmdir(profile))
        await ignoreMissing(rmdir(scratch))
        await removeScratchFixture(fixture)
      })
      await mkdir(retained)
      await writeFile(retainedFile, 'abc')
      await fs.symlink(other, linkedDirectory, 'junction')
      await fs.symlink(other, linkedScratch, 'junction')
    },
    (message) => reports.push(message),
  )
  assert.ok(reports.includes('Retained web-terminal-* directories: 2 (8 bytes)'))
  assert.equal((await lstat(otherFile)).size, 18)
})

async function createScratchFixture() {
  const base = join(root, '.test-tmp')
  await mkdir(base, { recursive: true })
  const fixture = await mkdtemp(join(base, 'scratch-test-'))
  await mkdir(join(fixture, '.test-tmp'))
  return fixture
}

async function removeScratchFixture(fixture) {
  await rmdir(join(fixture, '.test-tmp'))
  await rmdir(fixture)
}

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

  reply(id, result) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ id, result }) }))
  }
}

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
