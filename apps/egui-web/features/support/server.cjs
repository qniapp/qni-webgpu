const { spawn } = require('node:child_process')
const path = require('node:path')
const { once } = require('node:events')

const { getWebServerConfig } = require('../../test-support/web-server.cjs')

const APP_ROOT = path.join(__dirname, '..', '..')
const POLL_INTERVAL_MS = 250

let managedServerProcess = null

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

const probeServer = async (url) => {
  try {
    const response = await fetch(url, { redirect: 'manual' })
    await response.arrayBuffer().catch(() => {})
    return true
  } catch {
    return false
  }
}

const waitForServer = async (url, timeout) => {
  const deadline = Date.now() + timeout
  while (Date.now() < deadline) {
    if (await probeServer(url)) {
      return
    }
    await delay(POLL_INTERVAL_MS)
  }

  throw new Error(`Timed out waiting for egui-web test server: ${url}`)
}

const ensureSharedWebServer = async () => {
  const config = getWebServerConfig()

  if (managedServerProcess && managedServerProcess.exitCode === null) {
    await waitForServer(config.url, config.timeout)
    return { ...config, managed: true }
  }

  if (await probeServer(config.url)) {
    return { ...config, managed: false }
  }

  managedServerProcess = spawn('sh', ['-lc', config.command], {
    cwd: APP_ROOT,
    stdio: 'ignore',
  })

  try {
    await waitForServer(config.url, config.timeout)
  } catch (error) {
    await shutdownSharedWebServer()
    throw error
  }

  return { ...config, managed: true }
}

const shutdownSharedWebServer = async () => {
  const processToStop = managedServerProcess
  managedServerProcess = null

  if (!processToStop || processToStop.exitCode !== null) {
    return
  }

  processToStop.kill('SIGTERM')
  await Promise.race([once(processToStop, 'exit'), delay(5_000)])

  if (processToStop.exitCode === null) {
    processToStop.kill('SIGKILL')
    await once(processToStop, 'exit').catch(() => {})
  }
}

module.exports = {
  getSharedWebServerConfig: getWebServerConfig,
  ensureSharedWebServer,
  shutdownSharedWebServer,
}
