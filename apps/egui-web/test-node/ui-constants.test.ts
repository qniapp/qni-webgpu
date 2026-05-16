import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import test from 'node:test'

test('generated UI constants are in sync with Rust constants', () => {
  const result = spawnSync(
    process.execPath,
    ['-r', 'ts-node/register/transpile-only', 'scripts/generate-ui-constants.ts', '--check'],
    { cwd: process.cwd(), encoding: 'utf8' },
  )

  assert.equal(result.status, 0, result.stderr || result.stdout)
})
