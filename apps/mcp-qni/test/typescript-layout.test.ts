import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

type PackageJson = {
  scripts: Record<string, string>
}

const testDir = path.dirname(fileURLToPath(import.meta.url))
const packageRoot = testDir.endsWith(`${path.sep}dist${path.sep}test`)
  ? path.resolve(testDir, '..', '..')
  : path.resolve(testDir, '..')

function packagePath(...segments: string[]): string {
  return path.join(packageRoot, ...segments)
}

function readPackageJson(): PackageJson {
  return JSON.parse(
    readFileSync(packagePath('package.json'), 'utf8')
  ) as PackageJson
}

test('MCP server source and tests are TypeScript-only', () => {
  assert.ok(existsSync(packagePath('src/circuit.ts')), 'src/circuit.ts exists')
  assert.ok(existsSync(packagePath('src/index.ts')), 'src/index.ts exists')
  assert.ok(
    existsSync(packagePath('test/circuit.test.ts')),
    'test/circuit.test.ts exists'
  )

  assert.equal(existsSync(packagePath('src/circuit.js')), false)
  assert.equal(existsSync(packagePath('src/index.js')), false)
  assert.equal(existsSync(packagePath('test/circuit.test.js')), false)
})

test('MCP package scripts build and run compiled TypeScript', () => {
  const packageJson = readPackageJson()

  assert.equal(packageJson.scripts.build, 'tsc -p tsconfig.json')
  assert.equal(packageJson.scripts.typecheck, 'tsc -p tsconfig.json --noEmit')
  assert.equal(packageJson.scripts.start, 'node dist/src/index.js')
  assert.equal(
    packageJson.scripts.test,
    'pnpm run build && node --test dist/test/*.test.js'
  )
})
