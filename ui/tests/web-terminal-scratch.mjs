import {
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  realpath,
  rmdir,
  unlink,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const unixSocketPathLimit = 103 // macOS leaves 103 bytes before the terminating NUL.
const scratchPrefix = 'web-terminal-'
const profileName = 'browser-profile' // Outside the prefix: reused, not retained.

async function entry(path) {
  return lstat(path, { bigint: true }).catch((error) => {
    if (error.code !== 'ENOENT') throw error
  })
}

function checkDirectory(current, original) {
  if (!current?.isDirectory() || current.dev !== original?.dev || current.ino !== original?.ino) {
    throw Object.assign(new Error('Browser test directory changed'), { code: 'DIRECTORY_CHANGED' })
  }
}

function cleanupFailure(error) {
  const code = error?.code
  return typeof code === 'string' && /^[A-Z][A-Z0-9_]*$/.test(code)
    ? `cleanup incomplete (${code})`
    : 'cleanup incomplete'
}

async function removeKnownFiles(files, notes) {
  for (const [path, type] of files) {
    try {
      if ((await entry(path))?.[type]()) await unlink(path)
    } catch (error) {
      notes.push(cleanupFailure(error))
    }
  }
}

async function cleanSubdirectory(path, original, files) {
  const current = await entry(path)
  if (!current) return []
  checkDirectory(current, original)
  const notes = []
  await removeKnownFiles(files, notes)
  try {
    const remaining = await readdir(path)
    if (remaining.length) notes.push(`${remaining.length} retained`)
    else if (!notes.length) await rmdir(path)
  } catch (error) {
    notes.push(cleanupFailure(error))
  }
  return notes
}

async function createSocketDirectory(paths, identities) {
  try {
    const parent = await realpath(tmpdir())
    const parentIdentity = await entry(parent)
    if (!parentIdentity?.isDirectory()) throw new Error('Invalid temporary directory')
    identities.socketParent = { path: parent, identity: parentIdentity }
    paths.socketDirectory = await mkdtemp(join(parent, 'ads-'))
    identities.socket = await entry(paths.socketDirectory)
    if (!identities.socket?.isDirectory()) throw new Error('Invalid socket directory')
  } catch (cause) {
    throw new Error(
      'Could not create a Unix socket directory; set TMPDIR to a writable directory',
      {
        cause,
      },
    )
  }
  const socketPath = join(paths.socketDirectory, `${paths.channelName}.sock`)
  if (Buffer.byteLength(socketPath) > unixSocketPathLimit) {
    throw new Error(`Unix socket path exceeds ${unixSocketPathLimit} bytes; set a shorter TMPDIR`)
  }
}

async function cleanSocketDirectory(paths, identities, report) {
  const { socketDirectory, channelName } = paths
  if (!socketDirectory) return

  let notes
  try {
    const { path, identity } = identities.socketParent
    checkDirectory(await entry(path), identity)
    notes = await cleanSubdirectory(socketDirectory, identities.socket, [
      [join(socketDirectory, `${channelName}.sock`), 'isSocket'],
    ])
  } catch (error) {
    notes = [cleanupFailure(error)]
  }
  if (notes.length) report(`Browser test Unix socket directory: ${notes.join(', ')}`)
}

async function cleanScratch(parent, paths, identities, report) {
  const { scratch, workdir, hostConfig, daemonConfig } = paths
  const notes = []
  try {
    checkDirectory(await entry(parent), identities.parent)
    checkDirectory(await entry(scratch), identities.scratch)
    await removeKnownFiles(
      [
        [hostConfig, 'isFile'],
        [daemonConfig, 'isFile'],
      ],
      notes,
    )
    const remaining = new Set(await readdir(scratch))
    if (remaining.delete('work')) {
      try {
        const workNotes = await cleanSubdirectory(workdir, identities.work, [])
        if (workNotes.length) notes.push(`work (${workNotes.join(', ')})`)
      } catch (error) {
        notes.push(`work (${cleanupFailure(error)})`)
      }
    }
    if (remaining.size) notes.push(`root (${remaining.size} retained)`)
    if (!notes.length) await rmdir(scratch)
  } catch (error) {
    notes.push(cleanupFailure(error))
  }
  if (notes.length) report(`Browser test files in ${scratch}: ${notes.join(', ')}`)
}

async function reportRetainedDirectories(parent, identity, report) {
  try {
    checkDirectory(await entry(parent), identity)
    let count = 0
    let entries = 0
    for (const name of await readdir(parent)) {
      if (!name.startsWith(scratchPrefix)) continue
      const path = join(parent, name)
      if (!(await entry(path))?.isDirectory()) continue
      count++
      entries += (await readdir(path)).length
    }
    if (count) {
      report(`Retained web-terminal-* directories: ${count} (${entries} first-level entries)`)
    }
  } catch (error) {
    report(`Retained web-terminal-* directories: inventory unavailable (${cleanupFailure(error)})`)
  }
}

// An interrupted run leaves the lock behind, and only a person can tell it from a live one.
async function lockBrowserProfile(lock) {
  try {
    await writeFile(lock, `${process.pid}\n`, { flag: 'wx' })
  } catch (cause) {
    if (cause.code !== 'EEXIST') throw cause
    throw new Error(
      `Browser test profile is locked by ${lock}. Wait for the other run to finish. ` +
        'If no browser test or test browser is running, delete that file and run again.',
      { cause },
    )
  }
}

async function prepareBrowserProfile(profile, devToolsPort) {
  await mkdir(profile).catch((error) => {
    if (error.code !== 'EEXIST') throw error
  })
  const identity = await entry(profile)
  if (!identity?.isDirectory()) {
    throw new Error(
      `Browser test profile is not a real directory: ${profile}. Move it aside and run again.`,
    )
  }
  // A port left by an earlier browser would be read before the new one writes its own.
  const port = await entry(devToolsPort)
  if (port?.isFile()) await unlink(devToolsPort)
  else if (port) throw new Error(`Not a file: ${devToolsPort}. Move it aside and run again.`)
  return identity
}

async function cleanBrowserProfile(profile, devToolsPort, identity, report) {
  const notes = []
  try {
    checkDirectory(await entry(profile), identity)
    await removeKnownFiles([[devToolsPort, 'isFile']], notes)
    if (!(await readdir(profile)).length) await rmdir(profile)
  } catch (error) {
    notes.push(cleanupFailure(error))
  }
  if (notes.length) report(`Browser test profile ${profile}: ${notes.join(', ')}`)
}

async function withBrowserProfile(parent, run, report) {
  const lock = join(parent, `${profileName}.lock`)
  await lockBrowserProfile(lock)
  try {
    const profile = join(parent, profileName)
    const devToolsPort = join(profile, 'DevToolsActivePort')
    const identity = await prepareBrowserProfile(profile, devToolsPort)
    try {
      return await run({ profile, devToolsPort })
    } finally {
      await cleanBrowserProfile(profile, devToolsPort, identity, report)
    }
  } finally {
    await unlink(lock).catch((error) => {
      report(`Browser test profile lock ${lock}: ${cleanupFailure(error)}; delete that file`)
    })
  }
}

export async function withTestDirectory(root, run, report = console.warn) {
  const parent = join(await realpath(root), '.test-tmp')
  await mkdir(parent, { recursive: true })
  const identities = { parent: await entry(parent) }
  if (!identities.parent?.isDirectory()) throw new Error('Invalid browser test directory')

  return withBrowserProfile(
    parent,
    (browser) => withScratchDirectory(parent, identities, browser, run, report),
    report,
  )
}

async function withScratchDirectory(parent, identities, browser, run, report) {
  const scratch = await mkdtemp(join(parent, scratchPrefix))
  const paths = {
    scratch,
    hostConfig: join(scratch, 'host.toml'),
    daemonConfig: join(scratch, 'daemon.toml'),
    workdir: join(scratch, 'work'),
    channelName: `ads-web-test-${process.pid}`,
    ...browser,
  }
  try {
    identities.scratch = await entry(scratch)
    if (process.platform !== 'win32') await createSocketDirectory(paths, identities)
    await mkdir(paths.workdir)
    identities.work = await entry(paths.workdir)
    return await run(paths)
  } finally {
    await cleanSocketDirectory(paths, identities, report)
    await cleanScratch(parent, paths, identities, report)
    await reportRetainedDirectories(parent, identities.parent, report)
  }
}
