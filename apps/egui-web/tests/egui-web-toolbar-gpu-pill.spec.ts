import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  readEguiError,
  sampleCanvasPixels,
  waitForStartupReady,
  type CanvasPixel,
  type PixelSamplePoint,
} from './support/egui-web-spec-helpers'

const FLEXOKI_TX_2: CanvasPixel = [111, 110, 105, 255] // Flexoki tx-2 #6F6E69
const FLEXOKI_TX_3: CanvasPixel = [183, 181, 172, 255] // Flexoki tx-3 #B7B5AC
const FLEXOKI_BG: CanvasPixel = [255, 252, 240, 255] // Flexoki bg #FFFCF0
const FLEXOKI_BG_2: CanvasPixel = [242, 240, 229, 255] // Flexoki bg-2 #F2F0E5
const FLEXOKI_RED_600: CanvasPixel = [175, 48, 41, 255] // Flexoki red-600 #AF3029
const FLEXOKI_GREEN_600: CanvasPixel = [102, 128, 11, 255] // Flexoki green-600 #66800B
const FLEXOKI_BLUE_600: CanvasPixel = [32, 94, 166, 255] // Flexoki blue-600 #205EA6

const CIRCUIT_PICKER_TOOLBAR_SHIFT = 98 // default auto-width picker trigger + toolbar gap-2
const TOOLBAR_PROBES: PixelSamplePoint[] = [
  { name: 'undoIcon', x: 24 + CIRCUIT_PICKER_TOOLBAR_SHIFT, y: 16 },
  { name: 'runIcon', x: 196 + CIRCUIT_PICKER_TOOLBAR_SHIFT, y: 22 },
  { name: 'statusDot', x: 235 + CIRCUIT_PICKER_TOOLBAR_SHIFT, y: 21 },
]

const execModeFocusRingProbePoints = (cssWidth: number): PixelSamplePoint[] => [
  { name: 'leftOutside', x: cssWidth - 140, y: 22 },
  { name: 'topOutside', x: cssWidth - 80, y: 1 },
  { name: 'bottomOutside', x: cssWidth - 80, y: 41 },
]

const switchToGpu = async (page: Page): Promise<void> => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  await page.mouse.click((box?.x ?? 0) + (box?.width ?? 1000) - 30, (box?.y ?? 0) + 23)
}

const setExternalGpuStatus = async (
  page: Page,
  status: Record<string, unknown>
): Promise<void> => {
  await page.evaluate((nextStatus) => {
    const setter = (window as any).__setExternalGpuStatus
    if (typeof setter !== 'function') {
      throw new Error('__setExternalGpuStatus hook missing')
    }
    setter(JSON.stringify(nextStatus))
  }, status)
  await page.waitForTimeout(120)
}

test('Local mode keeps edit utilities but hides the GPU run cluster', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const pixels = await sampleCanvasPixels(page, canvas, TOOLBAR_PROBES)

  const REM = 32
  const GATE_SIZE = REM
  const PALETTE_ROW_Y = 2.5 * REM
  const PALETTE_ROW_GAP = 8
  const PALETTE_PADDING_Y = 20
  const PALETTE_CIRCUIT_GAP = 48
  const paletteBottom = PALETTE_ROW_Y + GATE_SIZE * 2 + PALETTE_ROW_GAP + PALETTE_PADDING_Y
  const lineY = paletteBottom + PALETTE_CIRCUIT_GAP + GATE_SIZE / 2

  const toolbarPaletteScanPoints: PixelSamplePoint[] = Array.from({ length: 92 }, (_, y) => ({
    name: `toolbarPaletteY${y}`,
    x: 500,
    y,
  }))
  const layoutPixels = await sampleCanvasPixels(page, canvas, [
    { name: 'toolbarTopLeft', x: 1, y: 1 },
    { name: 'paletteCircuitGap', x: 500, y: paletteBottom + PALETTE_CIRCUIT_GAP / 2 },
    ...toolbarPaletteScanPoints,
  ])
  const isBg = (pixel: CanvasPixel): boolean => pixelRgbDistance(pixel, FLEXOKI_BG) < 10
  const gapStart = toolbarPaletteScanPoints.findIndex((point) => !isBg(layoutPixels[point.name]))
  const paletteTopIndex = toolbarPaletteScanPoints.findIndex(
    (point, index) => index > gapStart && isBg(layoutPixels[point.name])
  )
  expect({
    undoIconVisible: pixelRgbDistance(pixels.undoIcon, FLEXOKI_TX_3) < 60,
    runIconHidden: pixelRgbDistance(pixels.runIcon, FLEXOKI_BG) < 35,
    statusDotHidden: pixelRgbDistance(pixels.statusDot, FLEXOKI_BG) < 35,
    paletteCircuitGap: lineY - GATE_SIZE / 2 - paletteBottom,
    gapStartsAfterToolbar: gapStart > 0,
    paletteStartsAfterGap: paletteTopIndex > gapStart,
    toolbarHeight: paletteTopIndex - gapStart,
    toolbarTopLeftBg: pixelRgbDistance(layoutPixels.toolbarTopLeft, FLEXOKI_BG) < 10,
    paletteCircuitGapBg: pixelRgbDistance(layoutPixels.paletteCircuitGap, FLEXOKI_BG_2) < 50,
    eguiError: await readEguiError(page),
  }).toEqual({
    undoIconVisible: true,
    runIconHidden: true,
    statusDotHidden: true,
    paletteCircuitGap: 48,
    gapStartsAfterToolbar: true,
    paletteStartsAfterGap: true,
    toolbarHeight: 24,
    toolbarTopLeftBg: true,
    paletteCircuitGapBg: true,
    eguiError: null,
  })
})

test('Local and GPU mouse toggles do not leave a blue focus outline around the segment', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  const cssWidth = box?.width ?? 1000
  const probes = execModeFocusRingProbePoints(cssWidth)

  await page.mouse.click((box?.x ?? 0) + cssWidth - 30, (box?.y ?? 0) + 23)
  await page.waitForTimeout(180)
  const afterGpuClick = await sampleCanvasPixels(page, canvas, probes)

  await page.mouse.click((box?.x ?? 0) + cssWidth - 100, (box?.y ?? 0) + 23)
  await page.waitForTimeout(180)
  const afterLocalClick = await sampleCanvasPixels(page, canvas, probes)
  expect({
    gpuClickDistances: Object.values(afterGpuClick).map((pixel) => pixelRgbDistance(pixel, FLEXOKI_BG) < 35),
    localClickDistances: Object.values(afterLocalClick).map((pixel) => pixelRgbDistance(pixel, FLEXOKI_BG) < 35),
  }).toEqual({
    gpuClickDistances: Object.values(afterGpuClick).map(() => true),
    localClickDistances: Object.values(afterLocalClick).map(() => true),
  })
})

test('GPU toolbar renders idle, running, completed, and failed status colors from the test hook', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await switchToGpu(page)

  let pixels = await sampleCanvasPixels(page, canvas, TOOLBAR_PROBES)
  const idleProbe = {
    runIconVisible: pixelRgbDistance(pixels.runIcon, FLEXOKI_TX_2) < 60,
    statusDotNotIdleBlue: pixelRgbDistance(pixels.statusDot, FLEXOKI_BLUE_600) > 90,
  }

  await setExternalGpuStatus(page, { status: 'completed', durationMs: 1400 })
  pixels = await sampleCanvasPixels(page, canvas, TOOLBAR_PROBES)
  const completedProbe = pixelRgbDistance(pixels.statusDot, FLEXOKI_GREEN_600) < 80

  const failureProbes: boolean[] = []
  for (const failure of [
    { status: 'failed', failure: 'backend_offline', url: 'localhost:8081' },
    { status: 'failed', failure: 'unsupported_gate', gate: 'Spacer' },
    { status: 'failed', failure: 'http', statusCode: 502 },
  ]) {
    await setExternalGpuStatus(page, failure)
    pixels = await sampleCanvasPixels(page, canvas, TOOLBAR_PROBES)
    failureProbes.push(pixelRgbDistance(pixels.statusDot, FLEXOKI_RED_600) < 80)
  }

  await setExternalGpuStatus(page, { status: 'running' })
  const pulseSamples: CanvasPixel[] = []
  for (let i = 0; i < 4; i += 1) {
    await page.waitForTimeout(220)
    pulseSamples.push((await sampleCanvasPixels(page, canvas, TOOLBAR_PROBES)).statusDot)
  }
  const pulseDistances = pulseSamples.map((pixel) => pixelRgbDistance(pixel, FLEXOKI_BLUE_600))
  const paperDistances = pulseSamples.map((pixel) => pixelRgbDistance(pixel, FLEXOKI_BG))
  expect({
    idleProbe,
    completedProbe,
    failureProbes,
    pulseLeavesPaper: Math.max(...paperDistances) > 40,
    pulseAnimates: Math.max(...pulseDistances) - Math.min(...pulseDistances) > 20,
  }).toEqual({
    idleProbe: { runIconVisible: true, statusDotNotIdleBlue: true },
    completedProbe: true,
    failureProbes: [true, true, true],
    pulseLeavesPaper: true,
    pulseAnimates: true,
  })
})
