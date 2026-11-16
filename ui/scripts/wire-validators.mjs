import { readFileSync, readdirSync } from 'node:fs'
import Ajv2020 from 'ajv/dist/2020.js'
import addFormats from 'ajv-formats'
import standaloneCode from 'ajv/dist/standalone/index.js'

export function wireValidators() {
  const id = 'virtual:wire-validators'
  return {
    name: 'wire-validators',
    resolveId(source) {
      if (source === id) return `\0${id}`
    },
    load(source) {
      if (source !== `\0${id}`) return
      const directory = new URL('../../contracts/schemas/', import.meta.url)
      const ajv = new Ajv2020({
        strict: true,
        strictRequired: false,
        allowUnionTypes: true,
        code: { source: true, esm: true },
      })
      addFormats(ajv)
      const exports = {}
      for (const file of readdirSync(directory)) {
        if (!file.endsWith('.schema.json')) continue
        const schema = JSON.parse(readFileSync(new URL(file, directory), 'utf8'))
        ajv.addSchema(schema)
        if (
          /^(command\.(auth\.hello|events\.subscribe|session\.)|event\.session\.|contract\.local-api\.server-message)/.test(
            file,
          )
        ) {
          exports[schema.$id.replace(/[^a-zA-Z0-9]/g, '_')] = schema.$id
        }
      }
      const imports = new Map()
      const generated = standaloneCode(ajv, exports).replace(
        /require\("([^"]+)"\)(\.default)?/g,
        (_, module, defaultAccess) => {
          if (!imports.has(module)) imports.set(module, `runtime${imports.size}`)
          return imports.get(module) + (defaultAccess ? 'Default' : '')
        },
      )
      return (
        [...imports]
          .map(
            ([module, name]) =>
              `import ${name} from ${JSON.stringify(module + '.js')}; const ${name}Default = ${name}.default ?? ${name};`,
          )
          .join('\n') +
        '\n' +
        generated
      )
    },
  }
}
