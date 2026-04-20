const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')

const DEFAULT_READY_TIMEOUT_MS = 20_000
const DEFAULT_ARTIFACT_DIR = path.join(os.tmpdir(), 'qni-egui-web-cucumber')

const waitForAppReady = async (page, timeout = DEFAULT_READY_TIMEOUT_MS) => {
  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout }
  )
}

const readEguiError = async (page) => page.evaluate(() => window.__eguiError || null)

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
  waitForAppReady,
  readEguiError,
  openEguiApp,
  sanitizeArtifactSegment,
  getScenarioArtifactPath,
}
