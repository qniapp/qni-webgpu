import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import type { Locator, Page } from 'playwright'
import type { CanvasPixel, DragPreviewProbe, PixelSamplePoint, Point } from './support-types'

declare global {
  interface Window {
    __eguiReady?: boolean
    __eguiError?: unknown
    __eguiReadStateVector?: () => unknown[] | Promise<unknown[]>
  }
}

type CanvasBoundingBox = { x: number; y: number; width: number; height: number }
type CanvasContentStats = { nonBackground: number; sampledPixels: number }
type ReadCanvasContentStatsOptions = { path?: string; background?: readonly number[]; threshold?: number; step?: number }
type WaitForCanvasContentOptions = ReadCanvasContentStatsOptions & { timeout?: number; minNonBackground?: number }
type ScreenshotWithRetryOptions = NonNullable<Parameters<Locator['screenshot']>[0]> & { waitForStateVector?: boolean }
type PaletteGateCenterOptions = { gateSize?: number; gap?: number; rowY?: number; count?: number }
type DragPreviewProbeOptions = {
  gateIndex?: number; gateSize?: number; paletteGap?: number; paletteRowY?: number; paletteCount?: number
  stateCircleSize?: number; stateCircleGap?: number; stateCircleBottomMargin?: number; stateCount?: number; rem?: number
}
type ArtifactWorld = { artifactDir?: string } | null | undefined

export const DEFAULT_READY_TIMEOUT_MS = 20_000
export const DEFAULT_CANVAS_CONTENT_TIMEOUT_MS = 5_000
export const DEFAULT_ARTIFACT_DIR = path.join(os.tmpdir(), 'qni-egui-web-cucumber')
const DEFAULT_EVALUATE_ATTEMPTS = 3
const DEFAULT_POLL_INTERVAL_MS = 100
export const DEFAULT_MIN_NON_BACKGROUND_PIXELS = 40
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

const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))
const includesAny = (value: unknown, substrings: readonly string[]): boolean =>
  substrings.some((substring) => String(value).includes(substring))

export const waitForAppReady = async (page: Page, timeout = DEFAULT_READY_TIMEOUT_MS): Promise<void> => {
  await page.waitForFunction(() => window.__eguiReady === true || Boolean(window.__eguiError), null, { timeout })
}

export const evaluateWithRetry = async <Result = unknown, Arg = unknown>(
  page: Page, fn: (arg: Arg) => Result | Promise<Result>, arg?: Arg, attempts = DEFAULT_EVALUATE_ATTEMPTS
): Promise<Result> => {
  let lastError: unknown

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const evaluatePageFunction = page.evaluate.bind(page) as (pageFunction: unknown, pageArgument?: unknown) => Promise<Result>
      return await evaluatePageFunction(fn, arg)
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

const screenshotWithRetry = async (
  page: Page, locator: Locator, { waitForStateVector = false, ...options }: ScreenshotWithRetryOptions = {},
  attempts = DEFAULT_EVALUATE_ATTEMPTS
): Promise<Buffer> => {
  let lastError: unknown

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
      if (waitForStateVector) {
        await waitForStateVectorReady(page).catch(() => {})
      }
    }
  }

  throw lastError
}

export const readEguiError = async (page: Page): Promise<string | null> => {
  const error = await evaluateWithRetry<unknown>(page, () => window.__eguiError || null)
  return error == null ? null : String(error)
}

export const readStateVector = async (page: Page): Promise<unknown[]> =>
  evaluateWithRetry<unknown[]>(page, async () => {
    if (!window.__eguiReadStateVector) {
      return []
    }

    return window.__eguiReadStateVector()
  })

export const waitForStartupReady = async (
  page: Page,
  { timeout = DEFAULT_READY_TIMEOUT_MS, waitForStateVector = false }: { timeout?: number; waitForStateVector?: boolean } = {}
): Promise<unknown[] | null> => {
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

export const waitForStateVectorReady = async (page: Page, timeout = DEFAULT_READY_TIMEOUT_MS): Promise<unknown[]> => {
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

export const readCanvasContentStats = async (
  page: Page,
  locator: Locator,
  {
    path: screenshotPath,
    background = DEFAULT_BACKGROUND_RGB,
    threshold = DEFAULT_NON_BACKGROUND_THRESHOLD,
    step = DEFAULT_CANVAS_SAMPLE_STEP,
  }: ReadCanvasContentStatsOptions = {}
): Promise<CanvasContentStats> => {
  const screenshot = await screenshotWithRetry(
    page,
    locator,
    screenshotPath ? { path: screenshotPath } : {}
  )
  const base64 = screenshot.toString('base64')

  return evaluateWithRetry<CanvasContentStats, {
    base64: string
    background: readonly number[]
    threshold: number
    step: number
  }>(
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

export const waitForCanvasContent = async (
  page: Page,
  locator: Locator,
  {
    timeout = DEFAULT_CANVAS_CONTENT_TIMEOUT_MS,
    minNonBackground = DEFAULT_MIN_NON_BACKGROUND_PIXELS,
    ...options
  }: WaitForCanvasContentOptions = {}
): Promise<CanvasContentStats> => {
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

const requireCanvasBoundingBox = async (page: Page): Promise<CanvasBoundingBox> => {
  const box = await page.locator('#egui-canvas').boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  return box
}

export const sampleCanvasPixels = async (
  page: Page,
  locator: Locator,
  samples: PixelSamplePoint[]
): Promise<Record<string, CanvasPixel>> => {
  const screenshot = await screenshotWithRetry(page, locator, {
    type: 'png',
    waitForStateVector: true,
  })
  const base64 = screenshot.toString('base64')
  const box = await locator.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }

  return evaluateWithRetry<Record<string, CanvasPixel>, {
    base64: string
    samples: PixelSamplePoint[]
    cssWidth: number
    cssHeight: number
  }>(
    page,
    async ({ base64, samples, cssWidth, cssHeight }) => {
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

      const scaleX = img.width / cssWidth
      const scaleY = img.height / cssHeight

      return Object.fromEntries(
        samples.map(({ name, x, y }) => {
          const data = ctx.getImageData(
            Math.floor(x * scaleX),
            Math.floor(y * scaleY),
            1,
            1
          ).data
          return [name, Array.from(data)]
        })
      )
    },
    { base64, samples, cssWidth: box.width, cssHeight: box.height }
  )
}

export const dragPointer = async (
  page: Page,
  from: Point,
  to: Point,
  steps = 6,
  release = true
): Promise<void> => {
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

export const releasePointer = async (page: Page, at: Point): Promise<void> => {
  const box = await requireCanvasBoundingBox(page)
  const endX = box.x + at.x
  const endY = box.y + at.y
  await page.mouse.move(endX, endY)
  await page.mouse.up()
}

export const getPaletteGateCenter = (
  cssWidth: number,
  gateIndex: number,
  {
    gateSize = DEFAULT_GATE_SIZE,
    gap = DEFAULT_PALETTE_GAP,
    rowY = DEFAULT_PALETTE_ROW_Y,
    count = DEFAULT_PALETTE_COUNT,
  }: PaletteGateCenterOptions = {}
): Point => {
  const paletteWidth = count * gateSize + (count - 1) * gap
  const paletteStartX = cssWidth / 2 - paletteWidth / 2

  return {
    x: paletteStartX + gateIndex * (gateSize + gap) + gateSize / 2,
    y: rowY + gateSize / 2,
  }
}

export const getDragPreviewAboveStatePanelProbe = (
  cssWidth: number,
  cssHeight: number,
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
  }: DragPreviewProbeOptions = {}
): DragPreviewProbe => {
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

export const openEguiApp = async (
  page: Page,
  baseUrl: string,
  pathname = '/'
): Promise<string> => {
  const targetUrl = new URL(pathname, baseUrl).toString()
  await page.goto(targetUrl, { waitUntil: 'load' })
  return targetUrl
}

export const sanitizeArtifactSegment = (value: unknown): string => {
  const normalized = String(value || 'scenario')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80)

  return normalized || 'scenario'
}

export const getScenarioArtifactPath = async (
  world: ArtifactWorld,
  scenarioName: string | undefined,
  suffix: string
): Promise<string> => {
  const artifactDir = world?.artifactDir || DEFAULT_ARTIFACT_DIR
  await fs.mkdir(artifactDir, { recursive: true })
  return path.join(artifactDir, `${sanitizeArtifactSegment(scenarioName)}-${suffix}`)
}
