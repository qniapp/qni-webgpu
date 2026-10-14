const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const srcDir = path.resolve(__dirname, '../src')
const rustFiles = (dir: string): string[] =>
  fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry: any) => {
    const fullPath = path.join(dir, entry.name)
    if (entry.isDirectory()) return rustFiles(fullPath)
    return entry.isFile() && entry.name.endsWith('.rs') ? [fullPath] : []
  })

test('Tailwind size mappings live only in the token module', () => {
  const mapping = /\/\/.*\b(?:text-(?:xs|sm|base|lg|xl|\d+xl)\b|spacing-\d|leading-\d|rounded-(?:none|sm|md|lg|xl|\d+xl|full)\b|rounded\s*=|(?:[pm][xytrbl]?|gap(?:-[xy])?|space-[xy]|[wh]|min-[wh]|max-[wh]|size|stroke)-\d)/
  const offenders = rustFiles(srcDir)
    .filter((file: string) => file !== path.join(srcDir, 'tailwind.rs'))
    .flatMap((file: string) =>
      fs.readFileSync(file, 'utf8').split('\n').flatMap((line: string, index: number) =>
        mapping.test(line)
          ? [`${path.relative(srcDir, file)}:${index + 1}: ${line.trim()}`]
          : []
      )
    )
  assert.deepEqual(offenders, [])
})

export {}
