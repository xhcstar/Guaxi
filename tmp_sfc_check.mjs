import { readFileSync } from 'node:fs'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'

const files = process.argv.slice(2)
let failed = false

for (const file of files) {
  const source = readFileSync(file, 'utf8')
  const { descriptor, errors } = parse(source, { filename: file })
  if (errors.length) {
    failed = true
    console.error(`[parse] ${file}`)
    for (const e of errors) console.error('  ', e.message)
    continue
  }
  try {
    compileScript(descriptor, { id: 'check' })
  } catch (e) {
    failed = true
    console.error(`[script] ${file}\n  ${e.message}`)
    continue
  }
  const tpl = compileTemplate({
    source: descriptor.template?.content ?? '',
    filename: file,
    id: 'check',
  })
  if (tpl.errors.length) {
    failed = true
    console.error(`[template] ${file}`)
    for (const e of tpl.errors) console.error('  ', typeof e === 'string' ? e : e.message)
    continue
  }
  console.log(`ok ${file}`)
}

process.exit(failed ? 1 : 0)
