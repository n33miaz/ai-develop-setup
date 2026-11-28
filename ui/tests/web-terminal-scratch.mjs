import { lstat, mkdir, mkdtemp, readdir, realpath, rmdir, unlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const unixSocketPathLimit = 103 // macOS leaves 103 bytes before the terminating NUL.

async function entry(path) {
  return lstat(path, { bigint: true }).catch((error) => {
    if (error.code !== 'ENOENT') throw error
  })
}

async function isSameDirectory(path, original) {
  const current = await entry(path)
  return current?.isDirectory() && current.dev === original?.dev && current.ino === original?.ino
}

async function removeKnownFiles(files) {
  await Promise.allSettled(
    files.map(async ([path, type]) => {
      if ((await entry(path))?.[type]()) await unlink(path)
    }),
  )
}

async function cleanSubdirectory(path, original, files) {
  if (!(await isSameDirectory(path, original))) return 0
  await removeKnownFiles(files)
  const remaining = await readdir(path)
  if (!remaining.length) await rmdir(path)
  return remaining.length
}

// Chrome puts a Unix socket two levels below TMPDIR and aborts when the path does not fit.
export function browserEnvironment(env, { scratch, socketDirectory = scratch }) {
  return { ...env, TMPDIR: socketDirectory }
}

export async function withTestDirectory(root, run, report = console.warn) {
  const parent = join(await realpath(root), '.test-tmp')
  await mkdir(parent, { recursive: true })
  const parentIdentity = await entry(parent)
  if (!parentIdentity?.isDirectory()) throw new Error('Invalid browser test directory')

  const scratch = await mkdtemp(join(parent, 'web-terminal-'))
  const scratchIdentity = await entry(scratch)
  const channelName = `ads-web-test-${process.pid}`
  const socketName = `${channelName}.sock`
  const profile = join(scratch, 'browser')
  const paths = {
    scratch,
    hostConfig: join(scratch, 'host.toml'),
    daemonConfig: join(scratch, 'daemon.toml'),
    workdir: join(scratch, 'work'),
    channelName,
    profile,
    devToolsPort: join(profile, 'DevToolsActivePort'),
  }
  let workIdentity, profileIdentity, socketParent, socketParentIdentity, socketIdentity
  try {
    if (process.platform !== 'win32') {
      try {
        socketParent = await realpath(tmpdir())
        socketParentIdentity = await entry(socketParent)
        if (!socketParentIdentity?.isDirectory()) throw new Error('Invalid temporary directory')
        paths.socketDirectory = await mkdtemp(join(socketParent, 'ads-'))
        socketIdentity = await entry(paths.socketDirectory)
        if (!socketIdentity?.isDirectory()) throw new Error('Invalid socket directory')
      } catch (cause) {
        throw new Error(
          'Could not create a Unix socket directory; set TMPDIR to a writable directory',
          {
            cause,
          },
        )
      }
      if (Buffer.byteLength(join(paths.socketDirectory, socketName)) > unixSocketPathLimit) {
        throw new Error(
          `Unix socket path exceeds ${unixSocketPathLimit} bytes; set a shorter TMPDIR`,
        )
      }
    }
    await mkdir(paths.workdir)
    workIdentity = await entry(paths.workdir)
    await mkdir(profile)
    profileIdentity = await entry(profile)
    return await run(paths)
  } finally {
    const socketDirectory = paths.socketDirectory
    if (socketDirectory) {
      try {
        if (!(await isSameDirectory(socketParent, socketParentIdentity))) {
          throw new Error('Unix socket parent directory changed')
        }
        const count = await cleanSubdirectory(socketDirectory, socketIdentity, [
          [join(socketDirectory, socketName), 'isSocket'],
        ])
        if (count) report(`Browser test files in ${socketDirectory}: ${count} retained`)
        else if (await entry(socketDirectory)) throw new Error('Unix socket directory changed')
      } catch {
        report(`Browser test files in ${socketDirectory}: cleanup incomplete`)
      }
    }
    const retained = []
    try {
      if (
        !(await isSameDirectory(parent, parentIdentity)) ||
        !(await isSameDirectory(scratch, scratchIdentity))
      ) {
        throw new Error('Browser test directory changed')
      }
      await removeKnownFiles([
        [paths.hostConfig, 'isFile'],
        [paths.daemonConfig, 'isFile'],
      ])
      const remaining = new Set(await readdir(scratch))
      for (const [name, path, identity, files] of [
        ['browser', profile, profileIdentity, [[paths.devToolsPort, 'isFile']]],
        ['work', paths.workdir, workIdentity, []],
      ]) {
        if (!remaining.has(name)) continue
        try {
          const count = await cleanSubdirectory(path, identity, files)
          if (count) retained.push(`${name} (${count} retained)`)
          if (count || !(await entry(path))) remaining.delete(name)
        } catch {
          retained.push(`${name} (cleanup incomplete)`)
          remaining.delete(name)
        }
      }
      if (remaining.size) retained.push(`root (${remaining.size} retained)`)
      if (!retained.length) await rmdir(scratch)
    } catch {
      retained.push('cleanup incomplete')
    }
    if (retained.length) report(`Browser test files in ${scratch}: ${retained.join(', ')}`)
  }
}
