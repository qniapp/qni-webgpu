const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const srcDir = path.resolve(__dirname, '../src')
const allowedFiles = new Set([
  path.join(srcDir, 'colors.rs'),
  path.join(srcDir, 'shared.rs'),
])

const rustFiles = (dir: string): string[] =>
  fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry: any) => {
    const fullPath = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      return rustFiles(fullPath)
    }
    return entry.isFile() && entry.name.endsWith('.rs') ? [fullPath] : []
  })

test('egui render code gets colors from the theme definition', () => {
  const forbidden = [
    /Color32::from_(?:rgb|rgba|gray)/,
    /Color32::WHITE|Color32::BLACK|Color32::TRANSPARENT/,
    /color_rgba\(/,
  ]
  const offenders = rustFiles(srcDir)
    .filter((file: string) => !allowedFiles.has(file))
    .flatMap((file: string) => {
      const rel = path.relative(path.resolve(__dirname, '..'), file)
      return fs
        .readFileSync(file, 'utf8')
        .split('\n')
        .flatMap((line: string, index: number) =>
          forbidden.some((pattern: RegExp) => pattern.test(line))
            ? [`${rel}:${index + 1}: ${line.trim()}`]
            : []
        )
    })

  assert.deepEqual(offenders, [])
})

export {}
