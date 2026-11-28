import assert from 'node:assert/strict'
import { once } from 'node:events'
import fs from 'node:fs/promises'
import {
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rmdir,
  unlink,
  writeFile,
} from 'node:fs/promises'
import { syncBuiltinESMExports } from 'node:module'
import { createServer } from 'node:net'
import { dirname, join, sep } from 'node:path'
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
  const fixture = await createScratchFixture()
  let scratch
  await withTestDirectory(fixture, async (paths) => {
    scratch = paths.scratch
    t.after(async () => {
      for (const file of [paths.hostConfig, paths.daemonConfig, paths.devToolsPort]) {
        await ignoreMissing(unlink(file))
      }
      for (const directory of [paths.profile, paths.workdir, scratch]) {
        await ignoreMissing(rmdir(directory))
      }
      await removeScratchFixture(fixture)
    })
    for (const file of [paths.hostConfig, paths.daemonConfig, paths.devToolsPort]) {
      await writeFile(file, 'fixture')
    }
  })
  await assert.rejects(readdir(scratch), { code: 'ENOENT' })
  assert.deepEqual(await readdir(join(fixture, '.test-tmp')), [])
})

test('browser scratch preserves unexpected entries and the original failure', async (t) => {
  const fixture = await createScratchFixture()
  const reports = []
  const failure = new Error('browser failed')
  let paths
  await assert.rejects(
    withTestDirectory(
      fixture,
      async (layout) => {
        paths = layout
        const extras = [paths.profile, paths.workdir].map((path) => join(path, 'unexpected'))
        t.after(async () => {
          for (const file of [paths.hostConfig, paths.devToolsPort, ...extras]) {
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
          await removeScratchFixture(fixture)
        })
        await writeFile(paths.hostConfig, 'fixture')
        await writeFile(paths.devToolsPort, 'fixture')
        for (const extra of extras) await writeFile(extra, 'preserve')
        await mkdir(paths.daemonConfig)
        throw failure
      },
      (message) => reports.push(message),
    ),
    (error) => error === failure,
  )
  assert.deepEqual((await readdir(paths.scratch)).sort(), ['daemon.toml', 'work'])
  assert.deepEqual(await readdir(paths.profile), ['unexpected'])
  assert.equal(reports.length, 2)
  assert.ok(reports[0].includes(paths.scratch))
  assert.match(reports[0], /work \(1 retained\).*root \(1 retained\)/)
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

test('consecutive runs reuse one browser profile', async (t) => {
  const fixture = await createScratchFixture()
  const profile = join(await fs.realpath(fixture), '.test-tmp', 'browser-profile')
  const kept = join(profile, 'Preferences')
  const runs = []
  t.after(async () => {
    for (const { path } of runs) {
      if (path === profile) continue
      await ignoreMissing(unlink(join(path, 'Preferences')))
      await ignoreMissing(rmdir(path))
    }
    await ignoreMissing(unlink(kept))
    await ignoreMissing(rmdir(profile))
    await removeScratchFixture(fixture)
  })
  for (let run = 0; run < 2; run++) {
    await withTestDirectory(fixture, async (paths) => {
      const { ino } = await lstat(paths.profile, { bigint: true })
      runs.push({ path: paths.profile, ino })
      if (run === 0) await writeFile(join(paths.profile, 'Preferences'), 'kept')
    })
  }
  assert.deepEqual(runs[1], runs[0])
  assert.equal(runs[0].path, profile)
  assert.equal(await readFile(kept, 'utf8'), 'kept')
  assert.deepEqual(await readdir(join(fixture, '.test-tmp')), ['browser-profile'])
})

test('a locked browser profile is refused with instructions until released', async (t) => {
  const fixture = await createScratchFixture()
  const parent = join(fixture, '.test-tmp')
  const lock = join(parent, 'browser-profile.lock')
  t.after(async () => {
    await ignoreMissing(unlink(lock))
    await removeScratchFixture(fixture)
  })
  const refused = (error) =>
    error.message.includes('browser-profile.lock') &&
    error.message.includes('delete that file and run again')
  const unexpected = () => assert.fail('the locked profile was used')

  await withTestDirectory(fixture, async () => {
    const before = await readdir(parent)
    await assert.rejects(withTestDirectory(fixture, unexpected), refused)
    assert.deepEqual(await readdir(parent), before)
  })

  // What an interrupted run leaves behind.
  await writeFile(lock, 'interrupted')
  await assert.rejects(withTestDirectory(fixture, unexpected), refused)
  assert.equal(await readFile(lock, 'utf8'), 'interrupted')
  await unlink(lock)
  assert.equal(await withTestDirectory(fixture, () => 'ran'), 'ran')
})

test('a linked browser profile is refused and its target stays intact', async (t) => {
  const fixture = await createScratchFixture()
  const parent = join(fixture, '.test-tmp')
  const profile = join(parent, 'browser-profile')
  const target = join(fixture, 'target')
  const port = join(target, 'DevToolsActivePort')
  await mkdir(target)
  await writeFile(port, 'outside')
  await fs.symlink(target, profile, 'junction')
  t.after(async () => {
    await unlink(profile)
    await ignoreMissing(unlink(port))
    await rmdir(target)
    await removeScratchFixture(fixture)
  })
  await assert.rejects(
    withTestDirectory(fixture, () => assert.fail('the linked profile was used')),
    /not a real directory/,
  )
  assert.equal(await readFile(port, 'utf8'), 'outside')
  assert.ok((await lstat(profile)).isSymbolicLink())
  assert.deepEqual(await readdir(parent), ['browser-profile'])
})

test('a DevTools port left by an earlier run is not read', async (t) => {
  const fixture = await createScratchFixture()
  const profile = join(fixture, '.test-tmp', 'browser-profile')
  const port = join(profile, 'DevToolsActivePort')
  await mkdir(profile)
  await writeFile(port, '1234\n/devtools/browser/old')
  t.after(async () => {
    await ignoreMissing(unlink(port))
    await ignoreMissing(rmdir(profile))
    await removeScratchFixture(fixture)
  })
  await withTestDirectory(fixture, async (paths) => {
    // The path the browser test polls, and the one inside the profile it launches with.
    for (const path of [paths.devToolsPort, join(paths.profile, 'DevToolsActivePort'), port]) {
      await assert.rejects(readFile(path, 'utf8'), { code: 'ENOENT' })
    }
    assert.equal(await fs.realpath(paths.profile), await fs.realpath(profile))
  })
})

test('the retained report reads only the first level and follows no links', async (t) => {
  const fixture = await createScratchFixture()
  const parent = join(await fs.realpath(fixture), '.test-tmp')
  const old = join(parent, 'web-terminal-previous')
  const oldProfile = join(old, 'browser')
  const oldFiles = [join(old, 'host.toml'), join(oldProfile, 'payload')]
  const linked = join(parent, 'web-terminal-link')
  await mkdir(oldProfile, { recursive: true })
  for (const file of oldFiles) await writeFile(file, 'fixture')
  await fs.symlink(oldProfile, linked, 'junction')
  const visited = []
  const mocks = ['lstat', 'readdir'].map((name) => {
    const call = fs[name]
    return t.mock.method(fs, name, (path, ...rest) => {
      visited.push(path)
      return call(path, ...rest)
    })
  })
  const restore = () => {
    for (const mock of mocks) mock.mock.restore()
    syncBuiltinESMExports()
  }
  t.after(async () => {
    restore()
    await unlink(linked)
    for (const file of oldFiles) await unlink(file)
    await rmdir(oldProfile)
    await rmdir(old)
    await removeScratchFixture(fixture)
  })
  const reports = []
  syncBuiltinESMExports()
  await withTestDirectory(
    fixture,
    () => {},
    (message) => reports.push(message),
  )
  restore()
  assert.ok(visited.includes(old))
  const inside = (path) =>
    path === oldProfile || [oldProfile, linked].some((base) => path.startsWith(base + sep))
  assert.deepEqual(visited.filter(inside), [])
  assert.deepEqual(reports, ['Retained web-terminal-* directories: 1 (2 first-level entries)'])
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
