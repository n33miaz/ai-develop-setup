import { lstat, mkdir, mkdtemp, readdir, realpath, rmdir, unlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const unixSocketPathLimit = 103 // macOS leaves 103 bytes before the terminating NUL.
const scratchPrefix = 'web-terminal-'

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
  const { scratch, profile, workdir, devToolsPort, hostConfig, daemonConfig } = paths
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
    for (const [name, path, identity, files] of [
      ['browser', profile, identities.profile, [[devToolsPort, 'isFile']]],
      ['work', workdir, identities.work, []],
    ]) {
      if (!remaining.delete(name)) continue
      try {
        const subdirectoryNotes = await cleanSubdirectory(path, identity, files)
        if (subdirectoryNotes.length) notes.push(`${name} (${subdirectoryNotes.join(', ')})`)
      } catch (error) {
        notes.push(`${name} (${cleanupFailure(error)})`)
      }
    }
    if (remaining.size) notes.push(`root (${remaining.size} retained)`)
    if (!notes.length) await rmdir(scratch)
  } catch (error) {
    notes.push(cleanupFailure(error))
  }
  if (notes.length) report(`Browser test files in ${scratch}: ${notes.join(', ')}`)
}

async function directoryBytes(path) {
  const pending = [path]
  let bytes = 0n
  while (pending.length) {
    const current = pending.pop()
    const info = await entry(current)
    if (info?.isDirectory()) {
      for (const name of await readdir(current)) pending.push(join(current, name))
    } else if (info?.isFile()) {
      bytes += info.size
    }
  }
  return bytes
}

async function reportRetainedDirectories(parent, identity, report) {
  try {
    checkDirectory(await entry(parent), identity)
    let count = 0
    let bytes = 0n
    for (const name of await readdir(parent)) {
      if (!name.startsWith(scratchPrefix)) continue
      const path = join(parent, name)
      if (!(await entry(path))?.isDirectory()) continue
      count++
      bytes += await directoryBytes(path)
    }
    if (count) report(`Retained web-terminal-* directories: ${count} (${bytes} bytes)`)
  } catch (error) {
    report(`Retained web-terminal-* directories: inventory unavailable (${cleanupFailure(error)})`)
  }
}

export async function withTestDirectory(root, run, report = console.warn) {
  const parent = join(await realpath(root), '.test-tmp')
  await mkdir(parent, { recursive: true })
  const identities = { parent: await entry(parent) }
  if (!identities.parent?.isDirectory()) throw new Error('Invalid browser test directory')

  const scratch = await mkdtemp(join(parent, scratchPrefix))
  const profile = join(scratch, 'browser')
  const paths = {
    scratch,
    hostConfig: join(scratch, 'host.toml'),
    daemonConfig: join(scratch, 'daemon.toml'),
    workdir: join(scratch, 'work'),
    channelName: `ads-web-test-${process.pid}`,
    profile,
    devToolsPort: join(profile, 'DevToolsActivePort'),
  }
  try {
    identities.scratch = await entry(scratch)
    if (process.platform !== 'win32') await createSocketDirectory(paths, identities)
    await mkdir(paths.workdir)
    identities.work = await entry(paths.workdir)
    await mkdir(profile)
    identities.profile = await entry(profile)
    return await run(paths)
  } finally {
    await cleanSocketDirectory(paths, identities, report)
    await cleanScratch(parent, paths, identities, report)
    await reportRetainedDirectories(parent, identities.parent, report)
  }
}
