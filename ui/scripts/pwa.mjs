import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'

export function pwa() {
  return {
    name: 'ads-service-worker',
    apply: 'build',
    generateBundle: {
      order: 'post',
      handler(_options, bundle) {
        const assets = Object.keys(bundle)
          .filter((file) => /^assets\/[^/]+\.(?:js|css)$/.test(file))
          .sort()
        const worker = readFileSync(new URL('../src/pwa/worker.js', import.meta.url), 'utf8')
        const version = createHash('sha256')
          .update(JSON.stringify(assets))
          .update(worker)
          .digest('hex')
        this.emitFile({
          type: 'asset',
          fileName: 'service-worker.js',
          source: `const assets = ${JSON.stringify(assets)}
const version = ${JSON.stringify(version)}
${worker}`,
        })
      },
    },
  }
}
