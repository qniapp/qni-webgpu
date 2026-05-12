import { expect, test } from '@playwright/test'
import { chromium } from 'playwright'
import type { Page } from 'playwright'
import { assertDragPreviewAboveOverlay } from '../features/support/assertions'
import {
  dragPointer,
  getDragPreviewAboveStatePanelProbe,
  getPaletteGateCenter,
  readBlochVectors,
  readEguiError,
  readMeasurementOutcomes,
  readStateVector,
  releasePointer,
  sampleCanvasPixels,
  waitForAppReady,
  waitForCanvasContent,
  waitForStartupReady,
} from '../features/support/egui-helpers'
import type { BlochEntry } from '../features/support/egui-helpers'
import type { CanvasPixel, PixelSamplePoint, Point } from '../features/support/support-types'
import { getPlainChromiumLaunchOptions } from '../test-support/browser-launch'
import { getWebServerConfig } from '../test-support/web-server'

type CircularBodySignature = {
  count: number
  width: number
  height: number
  samples: Record<string, CanvasPixel>
}

// Flexoki purple-600 — drag preview / semantic-intermediate fill.
const DRAG_PREVIEW_FILL: CanvasPixel = [94, 64, 157, 255]

const pixelRgbDistance = (left: CanvasPixel, right: CanvasPixel): number =>
  [0, 1, 2].reduce((total, channel) => total + Math.abs(left[channel] - right[channel]), 0)

const isDragPreviewFill = (pixel: CanvasPixel): boolean =>
  pixelRgbDistance(pixel, DRAG_PREVIEW_FILL) <= 80

const isRegularGateFill = ([r, g, b]: CanvasPixel): boolean =>
  r >= 35 && r <= 130 && g >= 120 && g <= 210 && b >= 100 && b <= 190

const isGateBodyFill = (pixel: CanvasPixel): boolean =>
  isRegularGateFill(pixel) || isDragPreviewFill(pixel)

const waitForStateVectorLength = async (
  page: Page,
  length: number,
  timeout = 5000
): Promise<void> => {
  await expect
    .poll(async () => (await readStateVector(page)).length, { timeout })
    .toBe(length)
}

const waitForStateVectorApprox = async (
  page: Page,
  expected: number[],
  timeout = 5000,
  tolerance = 1e-3
): Promise<void> => {
  await expect
    .poll(async () => {
      const actual = await readStateVector(page) as number[]
      if (actual.length !== expected.length) {
        return false
      }
      return expected.every((value, index) => Math.abs(actual[index] - value) < tolerance)
    }, { timeout })
    .toBe(true)
}

const waitForBlochVectorsApprox = async (
  page: Page,
  expected: Array<[number, number, number]>,
  timeout = 5000,
  tolerance = 1e-3
): Promise<void> => {
  await expect
    .poll(async () => {
      const entries = await readBlochVectors(page)
      if (entries.length !== expected.length) {
        return false
      }
      const sortedEntries = [...entries].sort((a: BlochEntry, b: BlochEntry) => a.gateId - b.gateId)
      return expected.every(([x, y, z], index) => {
        const e = sortedEntries[index]
        return (
          Math.abs(e.x - x) < tolerance &&
          Math.abs(e.y - y) < tolerance &&
          Math.abs(e.z - z) < tolerance
        )
      })
    }, { timeout })
    .toBe(true)
}

test('egui webgpu canvas renders content', async ({ page }, testInfo) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  const gpuAvailable = await page.evaluate(() => Boolean(navigator.gpu))
  expect(gpuAvailable).toBe(true)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const initialState = await readStateVector(page)
  expect(initialState).toEqual([1, 0, 0, 0])

  const initialRender = await waitForCanvasContent(page, canvas, {
    path: testInfo.outputPath('qni-egui-webgpu-initial.png'),
  })
  expect(initialRender.nonBackground).toBeGreaterThanOrEqual(40)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const hSource = getPaletteGateCenter(cssWidth, 0)
  const controlSource = getPaletteGateCenter(cssWidth, 14)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y
  const targetX2 = targetX + SLOT_SPACING
  const targetY2 = LINE_Y + LINE_GAP

  await dragPointer(page, hSource, { x: targetX, y: targetY })

  const expected = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, controlSource, { x: targetX2, y: targetY })

  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, xSource, { x: targetX2, y: targetY2 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)

  const afterRender = await waitForCanvasContent(page, canvas, {
    path: testInfo.outputPath('qni-egui-webgpu-after.png'),
  })
  expect(afterRender.nonBackground).toBeGreaterThanOrEqual(40)
  await canvas.screenshot({ path: testInfo.outputPath('qni-egui-webgpu.png') })
})

test('H on q0 and q1 yields uniform superposition', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  await dragPointer(page, hSource, { x: targetX, y: targetY0 })

  const expectedAfterQ0 = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedAfterQ0)

  await dragPointer(page, hSource, { x: targetX, y: targetY1 })

  const expected = [0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0]
  await waitForStateVectorApprox(page, expected)
})

test('dragging does not grow state vector until drop', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  const targetY2 = LINE_Y + 2 * LINE_GAP

  await dragPointer(page, hSource, { x: targetX, y: targetY0 })

  await dragPointer(page, hSource, { x: targetX, y: targetY1 })
  await waitForStateVectorLength(page, 8)

  await dragPointer(page, { x: targetX, y: targetY0 }, { x: targetX, y: targetY2 }, 6, false)

  const lengthDuringDrag = (await readStateVector(page)).length
  expect(lengthDuringDrag).toBe(8)

  await releasePointer(page, { x: targetX, y: targetY2 })

  await waitForStateVectorLength(page, 16)
})

test('palette panel keeps its corners and shadow while dragging', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const PALETTE_SIZE = REM
  const PALETTE_GAP = 8
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_ROW_GAP = 8
  const PALETTE_PADDING_X = 16
  const PALETTE_PADDING_Y = 20
  const PALETTE_ROW1_COUNT = 13
  const PALETTE_ROW2_COUNT = 8
  const row1Width = PALETTE_ROW1_COUNT * PALETTE_SIZE + (PALETTE_ROW1_COUNT - 1) * PALETTE_GAP
  const row2Width = PALETTE_ROW2_COUNT * PALETTE_SIZE + (PALETTE_ROW2_COUNT - 1) * PALETTE_GAP
  const paletteWidth = Math.max(row1Width, row2Width)
  const paletteHeight = 2 * PALETTE_SIZE + PALETTE_ROW_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const paletteRect = {
    x: paletteStartX - PALETTE_PADDING_X,
    y: PALETTE_ROW_Y - PALETTE_PADDING_Y,
    width: paletteWidth + PALETTE_PADDING_X * 2,
    height: paletteHeight + PALETTE_PADDING_Y * 2,
  }
  const hSource = getPaletteGateCenter(cssWidth, 0)
  const dragTarget = { x: hSource.x + 80, y: hSource.y + 80 }
  const panelPoints = [
    { name: 'corner', x: paletteRect.x + 2, y: paletteRect.y + 2 },
    { name: 'fill', x: paletteRect.x + 24, y: paletteRect.y + 24 },
    { name: 'shadow', x: paletteRect.x + paletteRect.width / 2, y: paletteRect.y + paletteRect.height + 10 },
    { name: 'background', x: paletteRect.x - 20, y: paletteRect.y + paletteRect.height + 10 },
  ]

  const beforeDrag = await sampleCanvasPixels(page, canvas, panelPoints)

  await dragPointer(page, hSource, dragTarget, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, panelPoints)

  for (const name of ['corner', 'fill']) {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    expect(diff).toBeLessThan(40)
  }

  expect(pixelRgbDistance(duringDrag.corner, duringDrag.fill)).toBeGreaterThan(10)

  const shadowBrightness = duringDrag.shadow[0] + duringDrag.shadow[1] + duringDrag.shadow[2]
  const backgroundBrightness = duringDrag.background[0] + duringDrag.background[1] + duringDrag.background[2]
  expect(Math.abs(shadowBrightness - backgroundBrightness)).toBeGreaterThan(10)

  await page.mouse.up()
})

test('palette control gate keeps its icon while dragging', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const PALETTE_SIZE = 32
  const dragSource = getPaletteGateCenter(cssWidth, 0)
  const dragTarget = { x: dragSource.x + 80, y: dragSource.y + 80 }
  const controlCenter = getPaletteGateCenter(cssWidth, 14)
  const controlRect = {
    x: controlCenter.x - PALETTE_SIZE / 2,
    y: controlCenter.y - PALETTE_SIZE / 2,
  }
  const signaturePoints = [
    { name: 'center', x: controlRect.x + PALETTE_SIZE / 2, y: controlRect.y + PALETTE_SIZE / 2 },
    { name: 'top', x: controlRect.x + PALETTE_SIZE / 2, y: controlRect.y + 6 },
    { name: 'bottom', x: controlRect.x + PALETTE_SIZE / 2, y: controlRect.y + PALETTE_SIZE - 6 },
    { name: 'left', x: controlRect.x + 6, y: controlRect.y + PALETTE_SIZE / 2 },
    { name: 'right', x: controlRect.x + PALETTE_SIZE - 6, y: controlRect.y + PALETTE_SIZE / 2 },
  ]

  const beforeDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await dragPointer(page, dragSource, dragTarget, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  for (const name of Object.keys(beforeDrag)) {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    expect(diff).toBeLessThan(40)
  }

  await page.mouse.up()
})

test('control gate uses the qni-style standalone circular dot', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const palettePos = getPaletteGateCenter(cssWidth, 14)
  const controlCenter = { x: palettePos.x, y: palettePos.y + 8 }
  const signaturePoints = [
    { name: 'center', x: controlCenter.x, y: controlCenter.y },
    { name: 'inner-left', x: controlCenter.x - 4, y: controlCenter.y },
    { name: 'inner-right', x: controlCenter.x + 4, y: controlCenter.y },
    { name: 'inner-top', x: controlCenter.x, y: controlCenter.y - 4 },
    { name: 'inner-bottom', x: controlCenter.x, y: controlCenter.y + 4 },
    { name: 'outside-left', x: controlCenter.x - 10, y: controlCenter.y },
    { name: 'outside-right', x: controlCenter.x + 10, y: controlCenter.y },
    { name: 'outside-top', x: controlCenter.x, y: controlCenter.y - 10 },
    { name: 'outside-bottom', x: controlCenter.x, y: controlCenter.y + 10 },
  ] satisfies PixelSamplePoint[]

  const pixels = await sampleCanvasPixels(page, canvas, signaturePoints)
  const isControlFill = ([r, g, b]: CanvasPixel): boolean => r < 90 && g > 120 && b > 100 && b < 180

  for (const name of ['center', 'inner-left', 'inner-right', 'inner-top', 'inner-bottom']) {
    expect(isControlFill(pixels[name]), `${name} should be filled by the control dot`).toBe(true)
  }
  for (const name of ['outside-left', 'outside-right', 'outside-top', 'outside-bottom']) {
    expect(isControlFill(pixels[name]), `${name} should remain outside the standalone circular dot`).toBe(false)
  }
})

test('anti-control gate uses the qni-style open circular dot', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const palettePos = getPaletteGateCenter(cssWidth, 15)
  const antiControlCenter = { x: palettePos.x, y: palettePos.y + 8 }
  const signaturePoints: PixelSamplePoint[] = [
    { name: 'center', x: antiControlCenter.x, y: antiControlCenter.y },
    { name: 'outside-left', x: antiControlCenter.x - 12, y: antiControlCenter.y },
    { name: 'outside-right', x: antiControlCenter.x + 12, y: antiControlCenter.y },
  ]
  for (let dy = -8; dy <= 8; dy += 2) {
    for (let dx = -8; dx <= 8; dx += 2) {
      if (dx === 0 && dy === 0) {
        continue
      }
      signaturePoints.push({ name: `ring-${dx},${dy}`, x: antiControlCenter.x + dx, y: antiControlCenter.y + dy })
    }
  }

  const pixels = await sampleCanvasPixels(page, canvas, signaturePoints)
  const isAntiControlStroke = ([r, g, b]: CanvasPixel): boolean => r < 90 && g > 120 && b > 100 && b < 180

  expect(isAntiControlStroke(pixels.center), 'center should remain open').toBe(false)
  const ringStrokeCount = signaturePoints
    .filter(({ name }) => name.startsWith('ring-'))
    .filter(({ name }) => isAntiControlStroke(pixels[name])).length
  expect(ringStrokeCount, 'open-circle stroke should be visible around the center').toBeGreaterThanOrEqual(8)
  for (const name of ['outside-left', 'outside-right']) {
    expect(isAntiControlStroke(pixels[name]), `${name} should remain outside the open circle`).toBe(false)
  }
})

test('control and anti-control have matching outer diameters', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const cssHeight = box?.height ?? 800

  const centerFor = (index: number): Point => {
    const pos = getPaletteGateCenter(cssWidth, index)
    return { x: pos.x, y: pos.y + 8 }
  }
  const screenshot = await canvas.screenshot({ type: 'png' })

  const bounds = await page.evaluate<
    Record<string, { width: number; height: number }>,
    { base64: string; cssWidth: number; cssHeight: number; centers: Record<string, Point> }
  >(async ({ base64, cssWidth: width, cssHeight: height, centers }) => {
    const img = new Image()
    img.src = `data:image/png;base64,${base64}`
    await new Promise((resolve, reject) => {
      img.onload = () => resolve(null)
      img.onerror = () => reject(new Error('Failed to decode screenshot'))
    })

    const probe = document.createElement('canvas')
    probe.width = img.width
    probe.height = img.height
    const ctx = probe.getContext('2d', { willReadFrequently: true })
    if (!ctx) {
      throw new Error('2D canvas unavailable')
    }
    ctx.drawImage(img, 0, 0)

    const scaleX = img.width / width
    const scaleY = img.height / height
    const sample = (x: number, y: number): CanvasPixel => {
      const data = ctx.getImageData(Math.floor(x * scaleX), Math.floor(y * scaleY), 1, 1).data
      return [data[0], data[1], data[2], data[3]]
    }
    const isControlGreen = ([r, g, b]: CanvasPixel): boolean => r < 90 && g > 120 && b > 100 && b < 180

    return Object.fromEntries(Object.entries(centers).map(([name, center]) => {
      let minX = Infinity
      let minY = Infinity
      let maxX = -Infinity
      let maxY = -Infinity
      for (let y = center.y - 12; y <= center.y + 12; y += 1) {
        for (let x = center.x - 12; x <= center.x + 12; x += 1) {
          if (!isControlGreen(sample(x, y))) {
            continue
          }
          minX = Math.min(minX, x)
          minY = Math.min(minY, y)
          maxX = Math.max(maxX, x)
          maxY = Math.max(maxY, y)
        }
      }
      return [name, { width: maxX - minX + 1, height: maxY - minY + 1 }]
    }))
  }, {
    base64: screenshot.toString('base64'),
    cssWidth,
    cssHeight,
    centers: {
      control: centerFor(14),
      antiControl: centerFor(15),
    },
  })

  expect(Math.abs(bounds.control.width - bounds.antiControl.width)).toBeLessThanOrEqual(1)
  expect(Math.abs(bounds.control.height - bounds.antiControl.height)).toBeLessThanOrEqual(1)
})

test('dragged palette gate stays visible above the palette panel', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const hSource = getPaletteGateCenter(cssWidth, 0)
  // Drop target sits outside the palette panel to the left, matching the
  // visual probe used in the original test.
  const dragTarget = {
    x: hSource.x - GATE_SIZE,
    y: hSource.y,
  }
  const dragRect = {
    x: dragTarget.x - GATE_SIZE / 2,
    y: dragTarget.y - GATE_SIZE / 2,
  }
  const fillPoint = {
    name: 'fill',
    x: dragRect.x + GATE_SIZE - 6,
    y: dragRect.y + GATE_SIZE - 6,
  }

  const beforeDrag = await sampleCanvasPixels(page, canvas, [fillPoint])

  await dragPointer(page, hSource, dragTarget, 6, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [fillPoint])

  const before = beforeDrag.fill
  const during = duringDrag.fill
  const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
  expect(diff).toBeGreaterThan(120)
  expect(isDragPreviewFill(during)).toBe(true)

  await page.mouse.up()
})

test('dragged palette gate stays above the state panel overlay', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  if (!box) {
    throw new Error('canvas bounding box should be available')
  }

  const { source, handleCenter, dragFillPoint, sourceFillPoint } =
    getDragPreviewAboveStatePanelProbe(box.width, box.height)
  const beforeDrag = await sampleCanvasPixels(page, canvas, [dragFillPoint, sourceFillPoint])

  await dragPointer(page, source, handleCenter, 8, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [dragFillPoint])

  assertDragPreviewAboveOverlay({
    before: beforeDrag.fill,
    during: duringDrag.fill,
    source: beforeDrag.sourceFill,
  })

  await page.mouse.up()
})

test('dragged palette gate keeps rounded corners', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const hSource = getPaletteGateCenter(cssWidth, 0)
  const dragTarget = { x: hSource.x + 80, y: hSource.y + 80 }
  const dragRect = {
    x: dragTarget.x - GATE_SIZE / 2,
    y: dragTarget.y - GATE_SIZE / 2,
  }

  await dragPointer(page, hSource, dragTarget, 6, false)
  await page.waitForTimeout(50)

  const pixels = await sampleCanvasPixels(page, canvas, [
    { name: 'corner', x: dragRect.x + 1, y: dragRect.y + 1 },
    { name: 'fill', x: dragRect.x + GATE_SIZE - 6, y: dragRect.y + GATE_SIZE - 6 },
  ])

  const cornerBrightness = pixels.corner[0] + pixels.corner[1] + pixels.corner[2]
  const fillBrightness = pixels.fill[0] + pixels.fill[1] + pixels.fill[2]
  expect(cornerBrightness).toBeGreaterThan(fillBrightness + 100)

  await page.mouse.up()
})

test('dragged x gate uses Flexoki purple-600 before dropping back to green', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const signaturePoints: PixelSamplePoint[] = []
  for (let dy = -24; dy <= 32; dy += 4) {
    for (let dx = -24; dx <= 32; dx += 4) {
      signaturePoints.push({ name: `${dx},${dy}`, x: targetCenter.x + dx, y: targetCenter.y + 8 + dy })
    }
  }

  await dragPointer(page, xSource, targetCenter, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await releasePointer(page, targetCenter)
  await page.waitForTimeout(50)
  const afterDrop = await sampleCanvasPixels(page, canvas, signaturePoints)

  const duringPurpleCount = Object.values(duringDrag).filter(isDragPreviewFill).length
  const afterGreenCount = Object.values(afterDrop).filter(isRegularGateFill).length
  expect(duringPurpleCount, 'dragged X body should use Flexoki purple-600').toBeGreaterThan(20)
  expect(afterGreenCount, 'dropped X body should return to regular green').toBeGreaterThan(20)
})

test('x gate uses a circular body in palette, circuit, and drag preview', async ({ page }, testInfo) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  await waitForCanvasContent(page, canvas)
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const xGateCenter = getPaletteGateCenter(cssWidth, 1)
  const placedXCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const dragXCenter = { x: placedXCenter.x + 80, y: placedXCenter.y + 40 }
  const readCircularBodySignature = async (center: Point): Promise<CircularBodySignature> => {
    const screenshot = await canvas.screenshot({ type: 'png' })
    const base64 = screenshot.toString('base64')
    const canvasBox = await canvas.boundingBox()
    expect(canvasBox).not.toBeNull()

    return page.evaluate<
      CircularBodySignature,
      { base64: string; center: Point; cssWidth: number; cssHeight: number }
    >(
      async ({ base64, center, cssWidth, cssHeight }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })

        const probe = document.createElement('canvas')
        probe.width = img.width
        probe.height = img.height
        const ctx = probe.getContext('2d', { willReadFrequently: true })
        if (!ctx) {
          throw new Error('2D canvas unavailable')
        }
        ctx.drawImage(img, 0, 0)

        const scaleX = img.width / cssWidth
        const scaleY = img.height / cssHeight
        const sample = (x: number, y: number): CanvasPixel => {
          const data = ctx.getImageData(Math.floor(x * scaleX), Math.floor(y * scaleY), 1, 1).data
          return [data[0], data[1], data[2], data[3]]
        }
        const dragPreviewFill = [94, 64, 157, 255]
        const rgbDistance = (left: CanvasPixel, right: CanvasPixel): number =>
          [0, 1, 2].reduce((total, channel) => total + Math.abs(left[channel] - right[channel]), 0)
        const isRegularGateFill = ([r, g, b]: CanvasPixel): boolean =>
          r >= 35 && r <= 130 && g >= 120 && g <= 210 && b >= 100 && b <= 190
        const isDragPreviewFill = (pixel: CanvasPixel): boolean =>
          rgbDistance(pixel, dragPreviewFill) <= 80
        const isFill = (pixel: CanvasPixel): boolean =>
          isRegularGateFill(pixel) || isDragPreviewFill(pixel)
        const searchRadius = 20
        let minX = Infinity
        let minY = Infinity
        let maxX = -Infinity
        let maxY = -Infinity
        let count = 0

        for (let y = center.y - searchRadius; y <= center.y + searchRadius; y += 1) {
          for (let x = center.x - searchRadius; x <= center.x + searchRadius; x += 1) {
            const pixel = sample(x, y)
            if (!isFill(pixel)) {
              continue
            }
            minX = Math.min(minX, x)
            minY = Math.min(minY, y)
            maxX = Math.max(maxX, x)
            maxY = Math.max(maxY, y)
            count += 1
          }
        }

        if (count === 0) {
          throw new Error('gate fill not found inside scoped X gate probe')
        }

        const midX = Math.floor((minX + maxX) / 2)
        const midY = Math.floor((minY + maxY) / 2)
        const insideRadius = 14
        const outsideDiagonal = 13
        return {
          count,
          width: maxX - minX + 1,
          height: maxY - minY + 1,
          samples: {
            top: sample(midX, midY - insideRadius),
            bottom: sample(midX, midY + insideRadius),
            left: sample(midX - insideRadius, midY),
            right: sample(midX + insideRadius, midY),
            topLeft: sample(midX - outsideDiagonal, midY - outsideDiagonal),
            topRight: sample(midX + outsideDiagonal, midY - outsideDiagonal),
            bottomLeft: sample(midX - outsideDiagonal, midY + outsideDiagonal),
            bottomRight: sample(midX + outsideDiagonal, midY + outsideDiagonal),
          },
        }
      },
      {
        base64,
        center,
        cssWidth: canvasBox?.width ?? 1000,
        cssHeight: canvasBox?.height ?? 800,
      }
    )
  }
  const expectCircularBody = async (label: string, center: Point): Promise<void> => {
    const signature = await readCircularBodySignature(center)
    const edgeNames = ['top', 'bottom', 'left', 'right']
    const cornerNames = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight']
    const filledEdges = edgeNames.filter((name) => isGateBodyFill(signature.samples[name])).length
    const filledCorners = cornerNames.filter((name) => isGateBodyFill(signature.samples[name])).length

    expect(signature.count, `${label} should find only the local X gate fill`).toBeGreaterThan(500)
    expect(signature.width, `${label} should have a full-width body`).toBeGreaterThan(24)
    expect(signature.height, `${label} should have a full-height body`).toBeGreaterThan(24)
    expect(
      filledEdges,
      `${label} should fill the four cardinal edge samples: ${JSON.stringify(signature)}`
    ).toBe(edgeNames.length)
    expect(
      filledCorners,
      `${label} should leave the four diagonal corner samples unfilled: ${JSON.stringify(signature)}`
    ).toBe(0)
  }

  await expectCircularBody('palette X gate', { x: xGateCenter.x, y: xGateCenter.y + 8 })

  await dragPointer(page, xGateCenter, placedXCenter)
  await page.waitForTimeout(50)
  await expectCircularBody('placed X gate', { x: placedXCenter.x + 8, y: placedXCenter.y + 8 })

  await dragPointer(page, placedXCenter, dragXCenter, 6, false)
  await page.waitForTimeout(50)
  await expectCircularBody('drag preview X gate', { x: dragXCenter.x + 24, y: dragXCenter.y + 16 })
  await canvas.screenshot({ path: testInfo.outputPath('x-gate-circular-body.png') })

  await page.mouse.up()
})

test('placed circuit gate keeps its visual while dragging another gate', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const sqrtXGateCenter = getPaletteGateCenter(cssWidth, 4)
  const hGateCenter = getPaletteGateCenter(cssWidth, 0)
  const placedGateCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const placedGateRect = {
    x: placedGateCenter.x - GATE_SIZE / 2,
    y: placedGateCenter.y - GATE_SIZE / 2,
  }

  const signaturePoints = []
  let pointIndex = 0
  for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 5; col++) {
      signaturePoints.push({
        name: `p${pointIndex++}`,
        x: placedGateRect.x + 5 + col * 5,
        y: placedGateRect.y + 5 + row * 5,
      })
    }
  }

  await dragPointer(page, sqrtXGateCenter, placedGateCenter)
  await page.waitForTimeout(50)
  await page.mouse.move((box?.x ?? 0) + placedGateCenter.x + 120, (box?.y ?? 0) + placedGateCenter.y + 120)
  await page.waitForTimeout(50)
  const beforeDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await dragPointer(page, hGateCenter, { x: placedGateCenter.x + 80, y: placedGateCenter.y + 40 }, 6, false)
  await page.waitForTimeout(50)
  const duringOtherDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  let totalDiff = 0
  for (const name of Object.keys(beforeDrag)) {
    const before = beforeDrag[name]
    const during = duringOtherDrag[name]
    totalDiff += Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
  }

  expect(totalDiff).toBeLessThan(1100)

  await page.mouse.up()
})

test('CNOT with control on q1 yields bell state', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const controlSource = getPaletteGateCenter(cssWidth, 14)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, hSource, { x: targetX, y: targetY1 })

  const expectedAfterH = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0, 0, 0, 0, 0]
  await waitForStateVectorApprox(page, expectedAfterH)

  await dragPointer(page, controlSource, { x: targetX2, y: targetY1 })

  await dragPointer(page, xSource, { x: targetX2, y: targetY0 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)
})

test('anti-control with a zero control wire applies the target gate', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const antiControlSource = getPaletteGateCenter(cssWidth, 15)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, antiControlSource, { x: targetX, y: targetY0 })
  await waitForStateVectorApprox(page, [1, 0, 0, 0])

  await dragPointer(page, xSource, { x: targetX, y: targetY1 })
  await waitForStateVectorApprox(page, [0, 0, 1, 0, 0, 0, 0, 0])
})

test('anti-control does not apply when the control wire is one', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const antiControlSource = getPaletteGateCenter(cssWidth, 15)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, xSource, { x: targetX, y: targetY0 })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await dragPointer(page, antiControlSource, { x: targetX2, y: targetY0 })
  await dragPointer(page, xSource, { x: targetX2, y: targetY1 })
  // anti-control(q0) sees q0=1, so it does not fire and the X on q1 is
  // suppressed. The state vector still grows to two qubits because q1 has a
  // placed gate; the final amplitude is on |q0=1, q1=0⟩ (state index 2 with
  // q0 as the MSB).
  await waitForStateVectorApprox(page, [0, 0, 0, 0, 1, 0, 0, 0])
})

test('Control does not affect gates in other columns', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const controlSource = getPaletteGateCenter(cssWidth, 14)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetX3 = targetX2 + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, hSource, { x: targetX, y: targetY0 })

  await dragPointer(page, controlSource, { x: targetX2, y: targetY0 })

  await dragPointer(page, xSource, { x: targetX3, y: targetY1 })

  const expected = [0, 0, 1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)
})

test('|0> resets a flipped qubit back to |0>', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const write0Source = getPaletteGateCenter(cssWidth, 17)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, xSource, { x: targetX, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await dragPointer(page, write0Source, { x: targetX2, y: targetY })
  await waitForStateVectorApprox(page, [1, 0, 0, 0])
})

test('|1> flips |0> to |1>', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const write1Source = getPaletteGateCenter(cssWidth, 18)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y

  await dragPointer(page, write1Source, { x: targetX, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])
})

test('|0> after H leaves the superposition unchanged (qni-faithful no-op)', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const write0Source = getPaletteGateCenter(cssWidth, 17)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, hSource, { x: targetX, y: targetY })
  const superposition = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, superposition)

  await dragPointer(page, write0Source, { x: targetX2, y: targetY })
  // qni's write gate is a no-op when the qubit is in superposition (pZero ≠ 0,1).
  await waitForStateVectorApprox(page, superposition)
})

test('Bloch display does not alter the state vector', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const blochSource = getPaletteGateCenter(cssWidth, 16)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, hSource, { x: targetX, y: targetY })
  const superposition = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, superposition)

  await dragPointer(page, blochSource, { x: targetX2, y: targetY })
  // BlochDisplay only reads the state; it must not mutate it.
  await waitForStateVectorApprox(page, superposition)
})

test('Measurement after X collapses the qubit to |1>', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const xSource = getPaletteGateCenter(cssWidth, 1)
  const measureSource = getPaletteGateCenter(cssWidth, 19)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, xSource, { x: targetX, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await dragPointer(page, measureSource, { x: targetX2, y: targetY })
  // Measurement collapses to a basis state. Since the pre-measurement
  // amplitude is fully on |1>, the only valid post-measurement vector is |1>.
  await waitForStateVectorApprox(page, [0, 0, 1, 0])
})

test('GPU compute pipeline applies a unitary chain end-to-end', async ({ page }) => {
  // Specifically targets the GPU per-gate compute path: a circuit with no
  // measurements should be simulated entirely by the WGSL `STATE_COMPUTE_SHADER`
  // dispatched once per linearised GateParams. We assert against the textbook
  // amplitudes for H q0 → CNOT(q0, q1) → Z q0 → H q0 (Bell-like prep with a
  // phase flip), which exercises matrix multiply + control mask + sign flip in
  // a single dispatch chain.
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const zSource = getPaletteGateCenter(cssWidth, 3)
  const controlSource = getPaletteGateCenter(cssWidth, 14)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetX3 = targetX2 + SLOT_SPACING
  const targetX4 = targetX3 + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  // H q0
  await dragPointer(page, hSource, { x: targetX, y: targetY0 })
  // CNOT(q0, q1) — control q0 + X q1 in slot 1
  await dragPointer(page, controlSource, { x: targetX2, y: targetY0 })
  await dragPointer(page, xSource, { x: targetX2, y: targetY1 })
  // Z q0 in slot 2
  await dragPointer(page, zSource, { x: targetX3, y: targetY0 })
  // H q0 in slot 3
  await dragPointer(page, hSource, { x: targetX4, y: targetY0 })

  // After H q0: (|00⟩+|10⟩)/√2 → CNOT: (|00⟩+|11⟩)/√2 → Z q0: (|00⟩-|11⟩)/√2
  // → H q0: (|00⟩-|01⟩+|10⟩+|11⟩)/2 (state index n = 2·q0 + q1; q0 is MSB).
  const half = 0.5
  await waitForStateVectorApprox(page, [half, 0, -half, 0, half, 0, half, 0])
})

test('GPU bloch reduction captures the textbook vectors per qubit', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const blochSource = getPaletteGateCenter(cssWidth, 16)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  // q0: H placed first → |+⟩ → bloch should report +x.
  await dragPointer(page, hSource, { x: targetX, y: targetY0 })
  await dragPointer(page, blochSource, { x: targetX2, y: targetY0 })
  // q1: X placed first → |1⟩ → bloch should report -z.
  await dragPointer(page, xSource, { x: targetX, y: targetY1 })
  await dragPointer(page, blochSource, { x: targetX2, y: targetY1 })

  await waitForBlochVectorsApprox(page, [
    [1, 0, 0],
    [0, 0, -1],
  ])
})

test('GPU measurement collapses |1> deterministically with outcome 1', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const xSource = getPaletteGateCenter(cssWidth, 1)
  const measureSource = getPaletteGateCenter(cssWidth, 19)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, xSource, { x: targetX, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])
  await dragPointer(page, measureSource, { x: targetX2, y: targetY })

  // pZero is exactly 0 because q0 = |1⟩, so the GPU sample and collapse must
  // converge on outcome=1 and a state of |1⟩ (the same amplitude as before
  // collapse, just normalized).
  await expect
    .poll(async () => {
      const outcomes = await readMeasurementOutcomes(page)
      if (outcomes.length !== 1) {
        return false
      }
      return outcomes[0].outcome === 1
    }, { timeout: 5000 })
    .toBe(true)
  await waitForStateVectorApprox(page, [0, 0, 1, 0])
})

test('Spacer is a NOP and does not alter the state vector', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const spacerSource = getPaletteGateCenter(cssWidth, 20)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, hSource, { x: targetX, y: targetY })
  const superposition = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, superposition)

  await dragPointer(page, spacerSource, { x: targetX2, y: targetY })
  await waitForStateVectorApprox(page, superposition)
})

test('default chromium shows a visible WebGPU error instead of a blank page', async () => {
  const plainChromium = getPlainChromiumLaunchOptions({
    env: process.env,
    defaultPath: chromium.executablePath(),
  })
  const { url } = getWebServerConfig()

  const browser = await chromium.launch(plainChromium)

  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } })
    await page.goto(new URL('/', url).toString(), { waitUntil: 'load' })
    await waitForAppReady(page)

    await expect.poll(async () => readEguiError(page), {
      timeout: 20000,
    }).not.toBeNull()
    await expect(page.locator('[data-testid="webgpu-error"]')).toBeVisible()
    await expect(page.locator('[data-testid="webgpu-error"]')).toContainText('WebGPU')
  } finally {
    await browser.close()
  }
})
