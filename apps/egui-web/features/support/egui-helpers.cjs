const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')

const DEFAULT_READY_TIMEOUT_MS = 20_000
const DEFAULT_CANVAS_CONTENT_TIMEOUT_MS = 5_000
const DEFAULT_ARTIFACT_DIR = path.join(os.tmpdir(), 'qni-egui-web-cucumber')
const DEFAULT_EVALUATE_ATTEMPTS = 3
const DEFAULT_POLL_INTERVAL_MS = 100
const DEFAULT_MIN_NON_BACKGROUND_PIXELS = 40
const DEFAULT_NON_BACKGROUND_THRESHOLD = 20
const DEFAULT_CANVAS_SAMPLE_STEP = 4
const DEFAULT_BACKGROUND_RGB = [255, 255, 255]

const RETRYABLE_EVALUATE_ERRORS = ['Execution context was destroyed']
const RETRYABLE_SCREENSHOT_ERRORS = [
  'Execution context was destroyed',
  'Element is not attached',
  'Cannot find context',
]

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const includesAny = (value, substrings) => substrings.some((substring) => String(value).includes(substring))

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
      if (!includesAny(error, RETRYABLE_EVALUATE_ERRORS)) {
        throw error
      }
      await page.waitForLoadState('load').catch(() => {})
      await waitForAppReady(page)
    }
  }

  throw lastError
}

const screenshotWithRetry = async (page, locator, options = {}, attempts = DEFAULT_EVALUATE_ATTEMPTS) => {
  let lastError

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      return await locator.screenshot({ type: 'png', ...options })
    } catch (error) {
      lastError = error
      if (!includesAny(error, RETRYABLE_SCREENSHOT_ERRORS)) {
        throw error
      }
      await page.waitForLoadState('load').catch(() => {})
      await waitForAppReady(page).catch(() => {})
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

const waitForStartupReady = async (
  page,
  { timeout = DEFAULT_READY_TIMEOUT_MS, waitForStateVector = false } = {}
) => {
  await waitForAppReady(page, timeout)

  const eguiError = await readEguiError(page)
  if (eguiError) {
    throw new Error(`egui app error while waiting for app startup: ${eguiError}`)
  }

  if (waitForStateVector) {
    return waitForStateVectorReady(page, timeout)
  }

  return null
}

const waitForStateVectorReady = async (page, timeout = DEFAULT_READY_TIMEOUT_MS) => {
  const deadline = Date.now() + timeout

  while (Date.now() < deadline) {
    const eguiError = await readEguiError(page)
    if (eguiError) {
      throw new Error(`egui app error while waiting for state vector: ${eguiError}`)
    }

    const stateVector = await readStateVector(page)
    if (stateVector.length > 0) {
      return stateVector
    }
    await delay(DEFAULT_POLL_INTERVAL_MS)
  }

  throw new Error('Timed out waiting for egui state vector to become available')
}

const readCanvasContentStats = async (
  page,
  locator,
  {
    path: screenshotPath,
    background = DEFAULT_BACKGROUND_RGB,
    threshold = DEFAULT_NON_BACKGROUND_THRESHOLD,
    step = DEFAULT_CANVAS_SAMPLE_STEP,
  } = {}
) => {
  const screenshot = await screenshotWithRetry(
    page,
    locator,
    screenshotPath ? { path: screenshotPath } : {}
  )
  const base64 = screenshot.toString('base64')

  return evaluateWithRetry(
    page,
    async ({ base64, background, threshold, step }) => {
      const img = new Image()
      img.src = `data:image/png;base64,${base64}`
      await new Promise((resolve, reject) => {
        img.onload = () => resolve(null)
        img.onerror = () => reject(new Error('Failed to decode screenshot'))
      })

      const canvas = document.createElement('canvas')
      canvas.width = img.width
      canvas.height = img.height
      const ctx = canvas.getContext('2d', { willReadFrequently: true })
      if (!ctx) {
        return { nonBackground: 0, sampledPixels: 0 }
      }
      ctx.drawImage(img, 0, 0)
      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
      const data = imageData.data
      const width = imageData.width
      const height = imageData.height
      let nonBackground = 0
      let sampledPixels = 0

      for (let y = 0; y < height; y += step) {
        for (let x = 0; x < width; x += step) {
          const idx = (y * width + x) * 4
          const diff =
            Math.abs(data[idx] - background[0]) +
            Math.abs(data[idx + 1] - background[1]) +
            Math.abs(data[idx + 2] - background[2])
          sampledPixels += 1
          if (diff > threshold) {
            nonBackground += 1
          }
        }
      }

      return { nonBackground, sampledPixels }
    },
    { base64, background, threshold, step }
  )
}

const waitForCanvasContent = async (
  page,
  locator,
  {
    timeout = DEFAULT_CANVAS_CONTENT_TIMEOUT_MS,
    minNonBackground = DEFAULT_MIN_NON_BACKGROUND_PIXELS,
    ...options
  } = {}
) => {
  const deadline = Date.now() + timeout
  let lastStats = { nonBackground: 0, sampledPixels: 0 }

  while (Date.now() < deadline) {
    const eguiError = await readEguiError(page)
    if (eguiError) {
      throw new Error(`egui app error while waiting for canvas content: ${eguiError}`)
    }

    lastStats = await readCanvasContentStats(page, locator, options)
    if (lastStats.nonBackground >= minNonBackground) {
      return lastStats
    }

    await delay(DEFAULT_POLL_INTERVAL_MS)
  }

  throw new Error(
    `Timed out waiting for egui canvas to render non-background content ` +
      `(nonBackground=${lastStats.nonBackground}, expected >= ${minNonBackground})`
  )
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
  DEFAULT_CANVAS_CONTENT_TIMEOUT_MS,
  DEFAULT_ARTIFACT_DIR,
  DEFAULT_MIN_NON_BACKGROUND_PIXELS,
  evaluateWithRetry,
  waitForAppReady,
  readEguiError,
  readStateVector,
  waitForStartupReady,
  waitForStateVectorReady,
  readCanvasContentStats,
  waitForCanvasContent,
  openEguiApp,
  sanitizeArtifactSegment,
  getScenarioArtifactPath,
}
