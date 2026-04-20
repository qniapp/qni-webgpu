const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')

const DEFAULT_READY_TIMEOUT_MS = 20_000
const DEFAULT_ARTIFACT_DIR = path.join(os.tmpdir(), 'qni-egui-web-cucumber')
const DEFAULT_EVALUATE_ATTEMPTS = 3
const DEFAULT_POLL_INTERVAL_MS = 100

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

const waitForAppReady = async (page, timeout = DEFAULT_READY_TIMEOUT_MS) => {
  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout }
  )
}

const evaluateWithRetry = async (page, fn, arg, attempts = DEFAULT_EVALUATE_ATTEMPTS) => {
  let lastError

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      return await page.evaluate(fn, arg)
    } catch (error) {
      lastError = error
      if (!String(error).includes('Execution context was destroyed')) {
        throw error
      }
      await page.waitForLoadState('load').catch(() => {})
      await waitForAppReady(page)
    }
  }

  throw lastError
}

const readEguiError = async (page) => evaluateWithRetry(page, () => window.__eguiError || null)

const readStateVector = async (page) =>
  evaluateWithRetry(page, async () => {
    if (!window.__eguiReadStateVector) {
      return []
    }

    return window.__eguiReadStateVector()
  })

const waitForStateVectorReady = async (page, timeout = DEFAULT_READY_TIMEOUT_MS) => {
  const deadline = Date.now() + timeout

  while (Date.now() < deadline) {
    const stateVector = await readStateVector(page)
    if (stateVector.length > 0) {
      return stateVector
    }
    await delay(DEFAULT_POLL_INTERVAL_MS)
  }

  throw new Error('Timed out waiting for egui state vector to become available')
}

const openEguiApp = async (page, baseUrl, pathname = '/') => {
  const targetUrl = new URL(pathname, baseUrl).toString()
  await page.goto(targetUrl, { waitUntil: 'load' })
  return targetUrl
}

const sanitizeArtifactSegment = (value) => {
  const normalized = String(value || 'scenario')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80)

  return normalized || 'scenario'
}

const getScenarioArtifactPath = async (world, scenarioName, suffix) => {
  const artifactDir = world?.artifactDir || DEFAULT_ARTIFACT_DIR
  await fs.mkdir(artifactDir, { recursive: true })
  return path.join(artifactDir, `${sanitizeArtifactSegment(scenarioName)}-${suffix}`)
}

module.exports = {
  DEFAULT_READY_TIMEOUT_MS,
  DEFAULT_ARTIFACT_DIR,
  evaluateWithRetry,
  waitForAppReady,
  readEguiError,
  readStateVector,
  waitForStateVectorReady,
  openEguiApp,
  sanitizeArtifactSegment,
  getScenarioArtifactPath,
}
