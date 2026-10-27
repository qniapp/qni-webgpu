import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import type { Locator, Page } from 'playwright'
import { UI_CONSTANTS } from '../../test-support/generated-ui-constants'
import { fullPaletteCircuitOffsetY, getPaletteGateCenter } from '../../test-support/palette-layout'
import type { CanvasPixel, DragPreviewProbe, PixelSamplePoint, Point } from './support-types'

export { getPaletteGateCenter }

// 全パレット・コメントブロックなしの横レイアウト用。
// ワイヤ座標はパネル内、セル座標はキャンバス内（幅 656px 以上）。
export const getCircuitWireY = (cssWidth: number, wire = 0): number =>
  UI_CONSTANTS.LINE_Y + fullPaletteCircuitOffsetY(cssWidth) + wire * UI_CONSTANTS.LINE_GAP

export const getCircuitCellCenter = (cssWidth: number, column = 0, wire = 0): Point => ({
  x: 8 + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + column * UI_CONSTANTS.SLOT_SPACING,
  y: 8 + getCircuitWireY(cssWidth, wire),
})

declare global {
  interface Window {
    __eguiReady?: boolean
    __eguiError?: unknown
    __eguiReadStateVector?: () => unknown[] | Promise<unknown[]>
    __eguiReadBlochVectors?: () => Promise<number[]>
    __eguiReadProbabilityDistributions?: () => Promise<number[]>
    __eguiReadAmplitudeCell?: (gateId: number, outcome: number) => Promise<number[]>
    __eguiReadDensityMatrixCell?: (gateId: number, row: number, col: number) => Promise<number[]>
    __eguiReadMeasurementOutcomes?: () => Promise<number[]>
  }
}

type CanvasBoundingBox = { x: number; y: number; width: number; height: number }
type CanvasContentStats = { nonBackground: number; sampledPixels: number }
type ReadCanvasContentStatsOptions = { path?: string; background?: readonly number[]; threshold?: number; step?: number }
type WaitForCanvasContentOptions = ReadCanvasContentStatsOptions & { timeout?: number; minNonBackground?: number }
type ScreenshotWithRetryOptions = NonNullable<Parameters<Locator['screenshot']>[0]> & { waitForStateVector?: boolean }
type ArtifactWorld = { artifactDir?: string } | null | undefined

// 起動が固まった場合、`bootstrap.ts` の監視が 15 秒後に一度だけ自動で読み込み
// 直す。その分の余裕を含めた待ち時間にする。
export const DEFAULT_READY_TIMEOUT_MS = 30_000
export const DEFAULT_CANVAS_CONTENT_TIMEOUT_MS = 5_000
export const DEFAULT_ARTIFACT_DIR = path.join(os.tmpdir(), 'qni-web-cucumber')
const DEFAULT_EVALUATE_ATTEMPTS = 3
const DEFAULT_POLL_INTERVAL_MS = 100
export const DEFAULT_MIN_NON_BACKGROUND_PIXELS = 40
const DEFAULT_NON_BACKGROUND_THRESHOLD = 20
const DEFAULT_CANVAS_SAMPLE_STEP = 4
const DEFAULT_BACKGROUND_RGB = [255, 255, 255]

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
  let onPageError: ((error: Error) => void) | undefined
  const pageError = new Promise<never>((_, reject) => {
    onPageError = (error) => reject(error)
    page.on('pageerror', onPageError)
  })
  try {
    await Promise.race([
      page.waitForFunction(() => window.__eguiReady === true || Boolean(window.__eguiError), null, { timeout }),
      pageError,
    ])
  } catch (error) {
    // 起動待ちの失敗は「どの段階で止まったか」が分からないと切り分けできない。
    // 起動段階フラグとキャンバスの実サイズを添えて投げ直す。
    const state = await page.evaluate(() => {
      const canvas = document.getElementById('egui-canvas')
      return {
        stage: Reflect.get(window, '__qniStartupStage') ?? 'not-started',
        ready: window.__eguiReady ?? null,
        error: window.__eguiError ? String(window.__eguiError).slice(0, 200) : null,
        canvasSize: canvas instanceof HTMLCanvasElement ? [canvas.width, canvas.height] : null,
      }
    })
    throw new Error(`egui app did not become ready: ${JSON.stringify(state)} :: ${String(error).split('\n')[0]}`)
  } finally {
    if (onPageError) page.off('pageerror', onPageError)
  }
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

export type BlochEntry = { gateId: number; x: number; y: number; z: number }

export const readBlochVectors = async (page: Page): Promise<BlochEntry[]> => {
  const flat = await evaluateWithRetry<number[]>(page, async () => {
    if (!window.__eguiReadBlochVectors) {
      return []
    }
    return await window.__eguiReadBlochVectors()
  })
  const entries: BlochEntry[] = []
  for (let i = 0; i + 3 < flat.length; i += 4) {
    entries.push({
      gateId: Math.round(flat[i]),
      x: flat[i + 1],
      y: flat[i + 2],
      z: flat[i + 3],
    })
  }
  return entries
}

export type ProbabilityDistributions = { gateId: number; probabilities: number[] }

export const readProbabilityDistributions = async (page: Page): Promise<ProbabilityDistributions[]> => {
  const flat = await evaluateWithRetry<number[]>(page, async () => {
    if (!window.__eguiReadProbabilityDistributions) {
      return []
    }
    return await window.__eguiReadProbabilityDistributions()
  })
  const valuesPerSlot = 65536
  const entries: ProbabilityDistributions[] = []
  for (let i = 0; i + valuesPerSlot < flat.length; i += valuesPerSlot + 1) {
    entries.push({
      gateId: Math.round(flat[i]),
      probabilities: flat.slice(i + 1, i + 1 + valuesPerSlot),
    })
  }
  return entries
}

export type AmplitudeCell = {
  gateId: number
  outcome: number
  re: number
  im: number
  incoherent: number
  quality: number
  phaseLockIndex: number
  span: number
}

export const readAmplitudeCell = async (page: Page, gateId: number, outcome: number): Promise<AmplitudeCell | null> => {
  const flat = await evaluateWithRetry<number[], { gateId: number; outcome: number }>(page, async ({ gateId, outcome }) => {
    if (!window.__eguiReadAmplitudeCell) {
      return []
    }
    return await window.__eguiReadAmplitudeCell(gateId, outcome)
  }, { gateId, outcome })
  if (flat.length < 8) {
    return null
  }
  return {
    gateId: Math.round(flat[0]),
    outcome: Math.round(flat[1]),
    re: flat[2],
    im: flat[3],
    incoherent: flat[4],
    quality: flat[5],
    phaseLockIndex: Math.round(flat[6]),
    span: Math.round(flat[7]),
  }
}

export type DensityMatrixCell = {
  gateId: number
  row: number
  col: number
  re: number
  im: number
  unity: number
  span: number
}

export const readDensityMatrixCell = async (page: Page, gateId: number, row: number, col: number): Promise<DensityMatrixCell | null> => {
  const flat = await evaluateWithRetry<number[], { gateId: number; row: number; col: number }>(page, async ({ gateId, row, col }) => {
    if (!window.__eguiReadDensityMatrixCell) {
      return []
    }
    return await window.__eguiReadDensityMatrixCell(gateId, row, col)
  }, { gateId, row, col })
  if (flat.length < 7) {
    return null
  }
  return {
    gateId: Math.round(flat[0]),
    row: Math.round(flat[1]),
    col: Math.round(flat[2]),
    re: flat[3],
    im: flat[4],
    unity: flat[5],
    span: Math.round(flat[6]),
  }
}

export type MeasurementOutcome = { gateId: number; outcome: number }

export const readMeasurementOutcomes = async (page: Page): Promise<MeasurementOutcome[]> => {
  const flat = await evaluateWithRetry<number[]>(page, async () => {
    if (!window.__eguiReadMeasurementOutcomes) {
      return []
    }
    return await window.__eguiReadMeasurementOutcomes()
  })
  const entries: MeasurementOutcome[] = []
  for (let i = 0; i + 1 < flat.length; i += 2) {
    entries.push({
      gateId: Math.round(flat[i]),
      outcome: Math.round(flat[i + 1]),
    })
  }
  return entries
}

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

export const getCircuitOriginY = async (page: Page): Promise<number> =>
  getCircuitWireY((await requireCanvasBoundingBox(page)).width)

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

export const getDragPreviewAboveStatePanelProbe = (
  cssWidth: number,
  cssHeight: number
): DragPreviewProbe => {
  const source = getPaletteGateCenter(cssWidth, 0)
  const gateSize = UI_CONSTANTS.GATE_SIZE

  // qni reference: `circle_notation` is rendered with `padding_x: 16,
  // padding_y: 20` (`apps/www/app/views/circuits/show.html.erb:66-67`),
  // matching the palette's `px-4 py-5`. Mirror that on the egui side.
  const statePaddingY = 20

  const columns = 4
  const rows = 1
  const size = 64
  const gap = 2

  const totalWidth = size * columns + gap * Math.max(columns - 1, 0)
  const totalHeight = size * rows + gap * Math.max(rows - 1, 0)
  const baseX = cssWidth / 2 - totalWidth / 2
  const baseY = cssHeight - UI_CONSTANTS.STATE_CIRCLE_BOTTOM_MARGIN - totalHeight
  const contentHeight = totalHeight + statePaddingY * 2
  const handleHeight = Math.max(Math.min(0.4 * UI_CONSTANTS.REM, contentHeight * 0.4), 10)
  const handlePadding = handleHeight * 0.5
  const handleCenter = {
    x: baseX + totalWidth / 2,
    y: baseY - (statePaddingY + handlePadding + handleHeight / 2),
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
