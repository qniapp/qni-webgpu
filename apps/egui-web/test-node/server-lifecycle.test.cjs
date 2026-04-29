const test = require('node:test')
const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')

const { terminateProcess } = require('../features/support/server.cjs')

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

test('terminateProcess resolves after a SIGTERM exit even when exitCode stays null', async (t) => {
  if (process.platform === 'win32') {
    t.skip('signal semantics differ on Windows')
    return
  }

  assert.equal(typeof terminateProcess, 'function')

  const child = spawn('sh', ['-lc', 'exec sleep 999'], { stdio: 'ignore' })

  t.after(() => {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL')
    }
  })

  await delay(100)

  const startedAt = Date.now()
  await terminateProcess(child, { termTimeoutMs: 250, killTimeoutMs: 250 })
  const elapsedMs = Date.now() - startedAt

  assert.ok(elapsedMs < 2_000, `terminateProcess took too long: ${elapsedMs}ms`)
  assert.equal(child.signalCode, 'SIGTERM')
})
