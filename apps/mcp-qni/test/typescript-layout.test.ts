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
  assert.deepEqual(
    {
      sourceCircuitTs: existsSync(packagePath('src/circuit.ts')),
      sourceIndexTs: existsSync(packagePath('src/index.ts')),
      testCircuitTs: existsSync(packagePath('test/circuit.test.ts')),
      sourceCircuitJs: existsSync(packagePath('src/circuit.js')),
      sourceIndexJs: existsSync(packagePath('src/index.js')),
      testCircuitJs: existsSync(packagePath('test/circuit.test.js')),
    },
    {
      sourceCircuitTs: true,
      sourceIndexTs: true,
      testCircuitTs: true,
      sourceCircuitJs: false,
      sourceIndexJs: false,
      testCircuitJs: false,
    }
  )
})

test('MCP package scripts build and run compiled TypeScript', () => {
  const packageJson = readPackageJson()

  assert.deepEqual(
    {
      build: packageJson.scripts.build,
      typecheck: packageJson.scripts.typecheck,
      start: packageJson.scripts.start,
      test: packageJson.scripts.test,
    },
    {
      build: 'tsc -p tsconfig.json',
      typecheck: 'tsc -p tsconfig.json --noEmit',
      start: 'node dist/src/index.js',
      test: 'pnpm run build && node --test dist/test/*.test.js',
    }
  )
})
