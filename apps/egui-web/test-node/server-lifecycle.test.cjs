const test = require('node:test')
const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const os = require('node:os')
const path = require('node:path')
const fs = require('node:fs/promises')

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

test('ensureSharedWebServer reuses an explicitly configured external server instead of spawning trunk', async (t) => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'qni-egui-external-server-'))
  const port = await new Promise((resolve, reject) => {
    const server = require('node:net').createServer()
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => resolve(address.port))
    })
    server.on('error', reject)
  })
  const serverProcess = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1', '--directory', tempDir], {
    stdio: 'ignore',
  })

  t.after(async () => {
    if (serverProcess.exitCode === null && serverProcess.signalCode === null) {
      serverProcess.kill('SIGKILL')
    }
    await fs.rm(tempDir, { recursive: true, force: true })
  })

  const serverModulePath = require.resolve('../features/support/server.cjs')
  delete require.cache[serverModulePath]

  const previousExternal = process.env.QNI_EGUI_WEB_EXTERNAL_SERVER
  const previousBaseUrl = process.env.QNI_EGUI_WEB_BASE_URL
  process.env.QNI_EGUI_WEB_EXTERNAL_SERVER = '1'
  process.env.QNI_EGUI_WEB_BASE_URL = `http://127.0.0.1:${port}`

  try {
    const { ensureSharedWebServer, shutdownSharedWebServer } = require('../features/support/server.cjs')
    const config = await ensureSharedWebServer()

    assert.equal(config.url, `http://127.0.0.1:${port}`)
    assert.equal(config.managed, false)
    await shutdownSharedWebServer()
  } finally {
    if (previousExternal === undefined) {
      delete process.env.QNI_EGUI_WEB_EXTERNAL_SERVER
    } else {
      process.env.QNI_EGUI_WEB_EXTERNAL_SERVER = previousExternal
    }

    if (previousBaseUrl === undefined) {
      delete process.env.QNI_EGUI_WEB_BASE_URL
    } else {
      process.env.QNI_EGUI_WEB_BASE_URL = previousBaseUrl
    }

    delete require.cache[serverModulePath]
  }
})
