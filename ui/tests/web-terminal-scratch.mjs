import { lstat, mkdir, mkdtemp, readdir, realpath, rmdir, unlink } from 'node:fs/promises'
import { join } from 'node:path'

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
  let workIdentity, profileIdentity
  try {
    await mkdir(paths.workdir)
    workIdentity = await entry(paths.workdir)
    await mkdir(profile)
    profileIdentity = await entry(profile)
    return await run(paths)
  } finally {
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
        [join(scratch, socketName), 'isSocket'],
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
