// Confere a licença de cada dependência direta (produção e desenvolvimento) contra a lista permitida.
// Mesma lista do deny.toml da raiz; GPL, AGPL, LGPL e MPL não entram sem ADR.
import { readFileSync } from 'node:fs'

const ALLOWED = new Set([
  'MIT',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  'Zlib',
  'Unicode-3.0',
])

const readJson = (url) => JSON.parse(readFileSync(new URL(url, import.meta.url), 'utf8'))

// Expressão SPDX simples: `A OR B` passa se uma alternativa passa; `A AND B` exige todas.
function isAllowed(expression) {
  if (typeof expression !== 'string' || expression.trim() === '') return false
  return expression
    .replace(/[()]/g, ' ')
    .split(/\s+OR\s+/i)
    .some((alternative) =>
      alternative.split(/\s+AND\s+/i).every((license) => ALLOWED.has(license.trim())),
    )
}

const manifest = readJson('../package.json')
const names = Object.keys({ ...manifest.dependencies, ...manifest.devDependencies }).sort()
const refused = []
for (const name of names) {
  const { license } = readJson(`../node_modules/${name}/package.json`)
  if (!isAllowed(license)) refused.push(`${name}: ${String(license)}`)
}

console.log(`dependências diretas conferidas: ${String(names.length)}`)
if (refused.length > 0) {
  console.error(`licença fora da lista permitida:\n  ${refused.join('\n  ')}`)
  process.exit(1)
}
