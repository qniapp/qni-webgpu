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
const DEFAULT_REM = 32
const DEFAULT_GATE_SIZE = 1 * DEFAULT_REM
const DEFAULT_PALETTE_GAP = 0.5 * DEFAULT_REM
const DEFAULT_PALETTE_ROW_Y = 2 * DEFAULT_REM
const DEFAULT_PALETTE_COUNT = 15
const DEFAULT_STATE_CIRCLE_SIZE = 1.25 * DEFAULT_REM
const DEFAULT_STATE_CIRCLE_GAP = 0.5 * DEFAULT_REM
const DEFAULT_STATE_CIRCLE_BOTTOM_MARGIN = 2 * DEFAULT_REM
const DEFAULT_STATE_COUNT = 4

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

const retryScreenshot = async (
  page,
  capture,
  { waitForStateVector = false } = {},
  attempts = DEFAULT_EVALUATE_ATTEMPTS
) => {
  let lastError

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      return await capture()
    } catch (error) {
      lastError = error
      if (!includesAny(error, RETRYABLE_SCREENSHOT_ERRORS)) {
        throw error
      }
      await page.waitForLoadState('load').catch(() => {})
      await waitForAppReady(page).catch(() => {})
      if (waitForStateVector) {
        await waitForStateVectorReady(page).catch(() => {})
      }
    }
  }

  throw lastError
}

const screenshotWithRetry = async (
  page,
  locator,
  { waitForStateVector = false, ...options } = {},
  attempts = DEFAULT_EVALUATE_ATTEMPTS
) =>
  retryScreenshot(
    page,
    () => locator.screenshot({ type: 'png', ...options }),
    { waitForStateVector },
    attempts
  )

const pageScreenshotWithRetry = async (
  page,
  { waitForStateVector = false, ...options } = {},
  attempts = DEFAULT_EVALUATE_ATTEMPTS
) =>
  retryScreenshot(
    page,
    () => page.screenshot({ type: 'png', ...options }),
    { waitForStateVector },
    attempts
  )

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

const requireCanvasBoundingBox = async (page) => {
  const box = await page.locator('#egui-canvas').boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  return box
}

const sampleCanvasPixels = async (page, locator, samples) => {
  const box = await locator.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  if (samples.length === 0) {
    return {}
  }

  const padding = 1
  const minX = Math.min(...samples.map(({ x }) => x))
  const maxX = Math.max(...samples.map(({ x }) => x))
  const minY = Math.min(...samples.map(({ y }) => y))
  const maxY = Math.max(...samples.map(({ y }) => y))
  const clipX = Math.max(0, Math.floor(minX) - padding)
  const clipY = Math.max(0, Math.floor(minY) - padding)
  const clipRight = Math.min(box.width, Math.ceil(maxX) + padding + 1)
  const clipBottom = Math.min(box.height, Math.ceil(maxY) + padding + 1)
  const clipWidth = Math.max(1, clipRight - clipX)
  const clipHeight = Math.max(1, clipBottom - clipY)

  const screenshot = await pageScreenshotWithRetry(page, {
    waitForStateVector: true,
    clip: {
      x: box.x + clipX,
      y: box.y + clipY,
      width: clipWidth,
      height: clipHeight,
    },
  })
  const base64 = screenshot.toString('base64')

  return evaluateWithRetry(
    page,
    async ({ base64, samples, clipX, clipY, clipWidth, clipHeight }) => {
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
        return {}
      }
      ctx.drawImage(img, 0, 0)

      const scaleX = img.width / clipWidth
      const scaleY = img.height / clipHeight
      const clamp = (value, max) => Math.min(Math.max(value, 0), Math.max(max - 1, 0))

      return Object.fromEntries(
        samples.map(({ name, x, y }) => {
          const data = ctx.getImageData(
            clamp(Math.floor((x - clipX) * scaleX), img.width),
            clamp(Math.floor((y - clipY) * scaleY), img.height),
            1,
            1
          ).data
          return [name, Array.from(data)]
        })
      )
    },
    { base64, samples, clipX, clipY, clipWidth, clipHeight }
  )
}

const dragPointer = async (page, from, to, steps = 6, release = true) => {
  const box = await requireCanvasBoundingBox(page)
  const startX = box.x + from.x
  const startY = box.y + from.y
  const endX = box.x + to.x
  const endY = box.y + to.y
  await page.mouse.move(startX, startY)
  await page.mouse.down()
  await page.waitForTimeout(16)
  await page.mouse.move(endX, endY, { steps })
  await page.waitForTimeout(16)
  if (release) {
    await page.mouse.up()
  }
}

const releasePointer = async (page, at) => {
  const box = await requireCanvasBoundingBox(page)
  const endX = box.x + at.x
  const endY = box.y + at.y
  await page.mouse.move(endX, endY)
  await page.mouse.up()
}

const getPaletteGateCenter = (
  cssWidth,
  gateIndex,
  {
    gateSize = DEFAULT_GATE_SIZE,
    gap = DEFAULT_PALETTE_GAP,
    rowY = DEFAULT_PALETTE_ROW_Y,
    count = DEFAULT_PALETTE_COUNT,
  } = {}
) => {
  const paletteWidth = count * gateSize + (count - 1) * gap
  const paletteStartX = cssWidth / 2 - paletteWidth / 2

  return {
    x: paletteStartX + gateIndex * (gateSize + gap) + gateSize / 2,
    y: rowY + gateSize / 2,
  }
}

const getDragPreviewAboveStatePanelProbe = (
  cssWidth,
  cssHeight,
  {
    gateIndex = 0,
    gateSize = DEFAULT_GATE_SIZE,
    paletteGap = DEFAULT_PALETTE_GAP,
    paletteRowY = DEFAULT_PALETTE_ROW_Y,
    paletteCount = DEFAULT_PALETTE_COUNT,
    stateCircleSize = DEFAULT_STATE_CIRCLE_SIZE,
    stateCircleGap = DEFAULT_STATE_CIRCLE_GAP,
    stateCircleBottomMargin = DEFAULT_STATE_CIRCLE_BOTTOM_MARGIN,
    stateCount = DEFAULT_STATE_COUNT,
    rem = DEFAULT_REM,
  } = {}
) => {
  const source = getPaletteGateCenter(cssWidth, gateIndex, {
    gateSize,
    gap: paletteGap,
    rowY: paletteRowY,
    count: paletteCount,
  })

  const statePadding = Math.min(rem, cssWidth * 0.05, cssHeight * 0.05)
  const topLimit = paletteRowY + gateSize + 2 * rem
  let availableWidth = cssWidth - statePadding * 2
  let availableHeight = cssHeight - stateCircleBottomMargin - topLimit
  if (availableWidth <= 0) {
    availableWidth = Math.max(cssWidth, 1)
  }
  if (availableHeight <= 0) {
    availableHeight = Math.max(cssHeight - stateCircleBottomMargin, 1)
  }

  const maxHeight = cssHeight * 0.4
  if (availableHeight > maxHeight) {
    availableHeight = Math.max(maxHeight, 1)
  }

  const gapRatio = stateCircleGap / stateCircleSize
  let columns = 1
  let rows = stateCount
  let bestSize = 0
  let bestScore = Number.POSITIVE_INFINITY
  const divisors = [1, 2, 4]
  for (const candidate of divisors) {
    if (stateCount % candidate !== 0) {
      continue
    }

    const candidateRows = stateCount / candidate
    const sizeW = availableWidth / (candidate + (candidate - 1) * gapRatio)
    const sizeH = availableHeight / (candidateRows + (candidateRows - 1) * gapRatio)
    const size = Math.min(sizeW, sizeH, stateCircleSize)
    const ratio = candidate / candidateRows
    const score = Math.abs(ratio - Math.max(availableWidth / availableHeight, 0.1))

    if (size > bestSize + 0.01 || (Math.abs(size - bestSize) <= 0.01 && score < bestScore)) {
      columns = candidate
      rows = candidateRows
      bestSize = size
      bestScore = score
    }
  }

  const size = Math.max(bestSize, 0.5)
  const gap = size * gapRatio
  const totalWidth = size * columns + gap * Math.max(columns - 1, 0)
  const totalHeight = size * rows + gap * Math.max(rows - 1, 0)
  const baseX = cssWidth / 2 - totalWidth / 2
  const baseY = cssHeight - stateCircleBottomMargin - totalHeight
  const contentHeight = totalHeight + statePadding * 2
  const handleHeight = Math.max(Math.min(0.4 * rem, contentHeight * 0.4), 10)
  const handlePadding = handleHeight * 0.5
  const handleCenter = {
    x: baseX + totalWidth / 2,
    y: baseY - (statePadding + handlePadding + handleHeight / 2),
  }

  return {
    source,
    handleCenter,
    sourceFillPoint: {
      name: 'sourceFill',
      x: source.x + gateSize / 2 - 6,
      y: source.y + gateSize / 2 - 6,
    },
    dragFillPoint: {
      name: 'fill',
      x: handleCenter.x + gateSize / 2 - 6,
      y: handleCenter.y + gateSize / 2 - 6,
    },
  }
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
  sampleCanvasPixels,
  dragPointer,
  releasePointer,
  getPaletteGateCenter,
  getDragPreviewAboveStatePanelProbe,
  openEguiApp,
  sanitizeArtifactSegment,
  getScenarioArtifactPath,
}
