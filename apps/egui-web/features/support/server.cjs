const { spawn } = require('node:child_process')
const path = require('node:path')
const { once } = require('node:events')

const { getWebServerConfig } = require('../../test-support/web-server.cjs')

const APP_ROOT = path.join(__dirname, '..', '..')
const POLL_INTERVAL_MS = 250
const PROCESS_TERM_TIMEOUT_MS = 5_000
const PROCESS_KILL_TIMEOUT_MS = 5_000

let managedServerProcess = null

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

const didProcessExit = (processToCheck) =>
  Boolean(processToCheck) && (processToCheck.exitCode !== null || processToCheck.signalCode !== null)

const waitForProcessExit = async (processToWaitFor, timeoutMs) => {
  if (!processToWaitFor || didProcessExit(processToWaitFor)) {
    return true
  }

  let timeoutId
  try {
    const timeout = new Promise((resolve) => {
      timeoutId = setTimeout(() => resolve(false), timeoutMs)
    })

    const exited = await Promise.race([
      once(processToWaitFor, 'exit')
        .then(() => true)
        .catch(() => true),
      timeout,
    ])

    return exited || didProcessExit(processToWaitFor)
  } finally {
    clearTimeout(timeoutId)
  }
}

const terminateProcess = async (
  processToStop,
  {
    termTimeoutMs = PROCESS_TERM_TIMEOUT_MS,
    killTimeoutMs = PROCESS_KILL_TIMEOUT_MS,
  } = {}
) => {
  if (!processToStop || didProcessExit(processToStop)) {
    return
  }

  processToStop.kill('SIGTERM')
  if (await waitForProcessExit(processToStop, termTimeoutMs)) {
    return
  }

  processToStop.kill('SIGKILL')
  await waitForProcessExit(processToStop, killTimeoutMs)
}

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

  if (managedServerProcess && !didProcessExit(managedServerProcess)) {
    await waitForServer(config.url, config.timeout)
    return { ...config, managed: true }
  }

  if (await probeServer(config.url)) {
    return { ...config, managed: false }
  }

  managedServerProcess = spawn('sh', ['-lc', `exec ${config.command}`], {
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

  await terminateProcess(processToStop)
}

module.exports = {
  getSharedWebServerConfig: getWebServerConfig,
  ensureSharedWebServer,
  shutdownSharedWebServer,
  terminateProcess,
}
