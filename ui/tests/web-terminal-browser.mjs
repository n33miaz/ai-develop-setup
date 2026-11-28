import assert from 'node:assert/strict'
import { access, constants, readFile, stat, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { cdpRequests, stop, until, withTestProcesses } from './web-terminal-harness.mjs'
import { withTestDirectory } from './web-terminal-scratch.mjs'

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

await withTestDirectory(root, runBrowser)

async function runBrowser(paths) {
  let socket
  try {
    await withTestProcesses(async (start) => {
      await writeTestConfigs(paths)
      const env = testEnvironment(paths.scratch)
      const { daemon, link } = await startTerminalServices(start, paths, env)
      const url = await startBrowser(start, paths, env)
      socket = new WebSocket(url)
      await waitForOpen(socket)
      const page = browserPage(cdpRequests(socket))
      await openTerminal(page, link)
      await checkKeyboardShortcuts(page)
      await stop(daemon.child)
      await checkRecovery(page, link)
      await checkSampleMode(page)
    })
  } finally {
    socket?.close()
  }
}

async function writeTestConfigs({ hostConfig, daemonConfig, workdir, channelName }) {
  await writeFile(hostConfig, `channel_name = '${channelName}'\n`)
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
}

function testEnvironment(scratch) {
  return {
    ...process.env,
    LOCALAPPDATA: scratch,
    TEMP: scratch,
    TMP: scratch,
    TMPDIR: scratch,
    XDG_RUNTIME_DIR: scratch,
    VITE_CORE_HARNESSES: 'claude',
  }
}

async function startTerminalServices(
  start,
  { scratch, hostConfig, daemonConfig, socketDirectory = scratch },
  env,
) {
  const channelEnv = { ...env, XDG_RUNTIME_DIR: socketDirectory }
  const host = start(binary('ads-ptyhost'), ['--config', hostConfig], { env: channelEnv })
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
    { env: channelEnv },
  )
  const link = await daemon.waitForReady(
    (output) => output.split(/\r?\n/)[1] !== undefined && output.trim(),
    'Test daemon failed to issue a link',
  )
  return { daemon, link }
}

async function startBrowser(start, { profile, devToolsPort }, env) {
  const browser = start(
    chromium,
    [
      '--headless',
      '--disable-gpu',
      '--disable-background-networking',
      '--lang=en-US',
      '--window-size=1366,800',
      '--no-first-run',
      '--remote-debugging-port=0',
      `--user-data-dir=${profile}`,
      'about:blank',
    ],
    { env },
  )
  const port = await browser.waitForReady(
    () =>
      readFile(devToolsPort, 'utf8')
        .then((data) => data.split('\n')[0])
        .catch(() => undefined),
    'Test browser failed to open DevTools',
  )
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
  return pages.find((page) => page.type === 'page').webSocketDebuggerUrl
}

function waitForOpen(socket) {
  return new Promise((ready, reject) => {
    socket.addEventListener('open', ready, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
}

function browserPage(send) {
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
  async function navigateToBlank() {
    await send('Page.navigate', { url: 'about:blank' })
    await until(() => evaluate("location.href === 'about:blank'"), 'Blank page did not load')
  }
  return { send, evaluate, text, hasText, state, navigateToBlank }
}

async function openTerminal({ send, evaluate, state, hasText }, link) {
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
}

async function checkKeyboardShortcuts({ send, evaluate, text, hasText }) {
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
}

async function checkRecovery({ send, state, text, hasText, navigateToBlank }, link) {
  await state('link_unavailable')
  assert.ok(!(await text()).includes('Sessions display received events'))
  await navigateToBlank()
  await send('Page.navigate', { url: link })
  await state('link_unavailable')
  await hasText('Open the new full link printed by the daemon')
  console.log('Interrupted connection and obsolete launch link show recovery guidance')
}

async function checkSampleMode({ send, evaluate, hasText, state, navigateToBlank }) {
  await evaluate('sessionStorage.clear()')
  await navigateToBlank()
  await send('Page.navigate', { url: origin })
  await hasText('Sample data only')
  await state('disconnected')
  console.log('Browser without a launch link shows sample guidance')
}
