import assert from 'node:assert/strict'
import { access, constants, mkdir, mkdtemp, readFile, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { cdpRequests, stop, until, withTestProcesses } from './web-terminal-harness.mjs'

const root = fileURLToPath(new URL('../..', import.meta.url))
const target = resolve(root, process.env.CARGO_TARGET_DIR ?? 'target', 'debug')
const windows = process.platform === 'win32'
const binary = (name) => join(target, name + (windows ? '.exe' : ''))
const chromium = process.env.CHROMIUM_BIN && resolve(process.env.CHROMIUM_BIN)

async function isExecutable(path) {
  try {
    await access(path, constants.X_OK)
    return (await stat(path)).isFile()
  } catch {
    return false
  }
}

const problems = []
if (!chromium || !(await isExecutable(chromium))) {
  const example = windows
    ? String.raw`$env:CHROMIUM_BIN = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'`
    : 'export CHROMIUM_BIN=/usr/bin/chromium'
  problems.push(
    chromium ? `Browser executable unavailable: ${chromium}` : 'CHROMIUM_BIN is unset.',
    `Set CHROMIUM_BIN to the path of your Chrome, Edge or Chromium executable, e.g.\n  ${example}`,
  )
}
const missing = []
for (const name of ['ads-ptyhost', 'ads-daemon']) {
  const path = binary(name)
  if (!(await isExecutable(path))) missing.push(path)
}
if (missing.length) {
  problems.push(
    `Debug executables unavailable:\n  ${missing.join('\n  ')}`,
    'From the repository root, run: cargo build -p ads-ptyhost -p ads-daemon',
  )
}
if (problems.length) throw new Error(problems.join('\n'))

const scratch = await mkdtemp(join(tmpdir(), 'ads-web-test-'))
const origin = 'http://127.0.0.1:5173'

async function control(socket, key, modifiers = 2) {
  const code = key === 'Insert' ? 'Insert' : 'Key' + key.toUpperCase()
  const virtualKey = key === 'Insert' ? 45 : key.toUpperCase().charCodeAt(0)
  for (const type of ['rawKeyDown', 'keyUp']) {
    await socket('Input.dispatchKeyEvent', {
      type,
      key,
      code,
      windowsVirtualKeyCode: virtualKey,
      modifiers,
    })
  }
}

let ws
await withTestProcesses(async (start) => {
  const hostConfig = join(scratch, 'host.toml')
  const daemonConfig = join(scratch, 'daemon.toml')
  const workdir = join(scratch, 'work')
  await mkdir(workdir)
  await writeFile(hostConfig, `channel_name = 'ads-web-test-${process.pid}'\n`)
  await writeFile(
    daemonConfig,
    `storage = false
[workdirs]
default = '${workdir}'
[tools.claude]
program = '${process.execPath}'
args = ['${fileURLToPath(new URL('terminal-byte-echo.cjs', import.meta.url))}']
`,
  )
  const env = {
    ...process.env,
    LOCALAPPDATA: scratch,
    XDG_RUNTIME_DIR: scratch,
    VITE_CORE_HARNESSES: 'claude',
  }
  const host = start(binary('ads-ptyhost'), ['--config', hostConfig], { env })
  await host.waitForReady(
    (output) => output.includes('listening on'),
    'Test terminal host failed to start',
  )
  const vite = start(process.execPath, ['node_modules/vite/bin/vite.js'], {
    cwd: join(root, 'ui'),
    env,
  })
  await vite.waitForReady(
    (output) => output.includes(origin),
    'Test Vite failed to start on port 5173',
  )
  const daemon = start(
    binary('ads-daemon'),
    ['--config', daemonConfig, '--host-config', hostConfig],
    { env },
  )
  const link = await daemon.waitForReady(
    (output) => output.split(/\r?\n/)[1] !== undefined && output.trim(),
    'Test daemon failed to issue a link',
  )

  const profile = join(scratch, 'browser')
  const browser = start(chromium, [
    '--headless',
    '--disable-gpu',
    '--disable-background-networking',
    '--lang=en-US',
    '--window-size=1366,800',
    '--no-first-run',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    'about:blank',
  ])
  const port = await browser.waitForReady(
    () =>
      readFile(join(profile, 'DevToolsActivePort'), 'utf8')
        .then((data) => data.split('\n')[0])
        .catch(() => undefined),
    'Test browser failed to open DevTools',
  )
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
  ws = new WebSocket(pages.find((page) => page.type === 'page').webSocketDebuggerUrl)
  await new Promise((ready, reject) => {
    ws.addEventListener('open', ready, { once: true })
    ws.addEventListener('error', reject, { once: true })
  })
  const send = cdpRequests(ws)
  async function evaluate(expression) {
    const result = await send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true,
    })
    assert.ok(!result.exceptionDetails, 'Browser evaluation failed')
    return result.result.value
  }
  const text = () => evaluate('document.body?.innerText')
  const hasText = (value) =>
    until(async () => (await text())?.includes(value), `Missing browser text: ${value}`)
  const state = (value) =>
    until(
      () => evaluate(`Boolean(document.querySelector('[data-state="${value}"]'))`),
      `Missing connection state: ${value}`,
    )
  await send('Browser.grantPermissions', {
    origin,
    permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'],
  })
  await send('Page.bringToFront')
  await send('Page.navigate', { url: link })
  await state('connected')
  await until(
    () =>
      evaluate(
        `[...document.querySelectorAll('button')].some(b => b.textContent.endsWith(' claude') && !b.disabled)`,
      ),
    'Tool button did not become available',
  )
  await evaluate(
    `[...document.querySelectorAll('button')].find(b => b.textContent.endsWith(' claude')).click()`,
  )
  await hasText('RAW READY')
  assert.equal(await evaluate('document.activeElement?.className'), 'xterm-helper-textarea')

  for (const [key, modifiers, value] of [
    ['v', 2, 'plain'],
    ['V', 10, 'shift'],
    ['Insert', 8, 'insert'],
  ]) {
    await evaluate(`navigator.clipboard.writeText('${value}')`)
    await control(send, key, modifiers)
    const payload = Buffer.from(`\x1b[200~${value}\x1b[201~`).toString('hex')
    await until(async () => {
      const screen = await text()
      assert.ok(!screen.includes('INPUT 16'), 'Substitute received ^V (0x16)')
      return screen.includes(`INPUT ${payload}`)
    }, `Substitute did not receive bracketed paste for ${value}`)
  }
  await control(send, 'c')
  await hasText('INPUT 03')

  // Select the first word in the terminal using xterm's mouse handling.
  const cell = await evaluate(`(() => {
    const screen = document.querySelector('.xterm-screen')
    const rect = screen.getBoundingClientRect()
    const row = screen.querySelector('.xterm-rows > div').getBoundingClientRect()
    return { x: rect.left + 4, y: row.top + row.height / 2 }
  })()`)
  for (const clickCount of [1, 2]) {
    for (const type of ['mousePressed', 'mouseReleased']) {
      await send('Input.dispatchMouseEvent', { ...cell, type, button: 'left', clickCount })
    }
  }
  await control(send, 'c')
  await until(
    () => evaluate(`navigator.clipboard.readText().then(value => value === 'RAW')`),
    'Ctrl+C did not copy selection',
  )
  assert.equal((await text()).match(/INPUT 03/g)?.length, 1)
  console.log('Browser keyboard shortcuts reached the substitute with the expected bytes')

  await stop(daemon.child)
  await state('link_unavailable')
  assert.ok(!(await text()).includes('Sessions display received events'))
  await send('Page.navigate', { url: 'about:blank' })
  await until(() => evaluate("location.href === 'about:blank'"), 'Blank page did not load')
  await send('Page.navigate', { url: link })
  await state('link_unavailable')
  await hasText('Open the new full link printed by the daemon')
  console.log('Interrupted connection and obsolete launch link show recovery guidance')

  await evaluate('sessionStorage.clear()')
  await send('Page.navigate', { url: 'about:blank' })
  await until(() => evaluate("location.href === 'about:blank'"), 'Blank page did not load')
  await send('Page.navigate', { url: origin })
  await hasText('Sample data only')
  await state('disconnected')
  console.log('Browser without a launch link shows sample guidance')
}).finally(() => ws?.close())
