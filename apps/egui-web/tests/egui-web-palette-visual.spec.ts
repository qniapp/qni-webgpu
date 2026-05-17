import { expect, test } from '@playwright/test'
import {
  chromium,
  dragPointer,
  getDragPreviewAboveStatePanelProbe,
  getPaletteGateCenter,
  getPlainChromiumLaunchOptions,
  getWebServerConfig,
  isDragPreviewFill,
  isGateBodyFill,
  isRegularGateFill,
  pixelRgbDistance,
  readEguiError,
  readMeasurementOutcomes,
  readStateVector,
  releasePointer,
  sampleCanvasPixels,
  waitForAppReady,
  waitForBlochVectorsApprox,
  waitForCanvasContent,
  waitForStartupReady,
  waitForStateVectorApprox,
  waitForStateVectorLength,
  type CanvasPixel,
  type CircularBodySignature,
  type PixelSamplePoint,
  type Point,
} from './support/egui-web-spec-helpers'

test('palette gate hover outline uses Flexoki purple-400', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  const gateCenter = getPaletteGateCenter(box.width, 0)
  await page.mouse.move(box.x + gateCenter.x, box.y + gateCenter.y)
  await page.waitForTimeout(50)
  const pixels = await sampleCanvasPixels(page, canvas, [{ name: 'hoverRing', x: gateCenter.x - 19, y: gateCenter.y }])

  expect(pixelRgbDistance(pixels.hoverRing, [139, 126, 200, 255])).toBeLessThan(48)
})

test('palette panel keeps its corners and shadow while dragging', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const PALETTE_SIZE = REM
  const PALETTE_GAP = 8
  const PALETTE_ROW_Y = 2.5 * REM
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

  const stablePanelParts = ['corner', 'fill'].map((name) => {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    return diff < 40
  })
  const shadowBrightness = duringDrag.shadow[0] + duringDrag.shadow[1] + duringDrag.shadow[2]
  const backgroundBrightness = duringDrag.background[0] + duringDrag.background[1] + duringDrag.background[2]
  expect({
    stablePanelParts,
    cornerDiffersFromFill: pixelRgbDistance(duringDrag.corner, duringDrag.fill) > 10,
    shadowVisible: Math.abs(shadowBrightness - backgroundBrightness) > 10,
  }).toEqual({ stablePanelParts: [true, true], cornerDiffersFromFill: true, shadowVisible: true })

  await page.mouse.up()
})

test('palette control gate keeps its icon while dragging', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
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

  const stableIconPixels = Object.keys(beforeDrag).map((name) => {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    return diff < 40
  })
  expect(stableIconPixels).toEqual(Object.keys(beforeDrag).map(() => true))

  await page.mouse.up()
})

test('control gate uses the qni-style standalone circular dot', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
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

  expect({
    filled: ['center', 'inner-left', 'inner-right', 'inner-top', 'inner-bottom'].map((name) => isControlFill(pixels[name])),
    outside: ['outside-left', 'outside-right', 'outside-top', 'outside-bottom'].map((name) => isControlFill(pixels[name])),
  }).toEqual({ filled: [true, true, true, true, true], outside: [false, false, false, false] })
})

test('anti-control gate uses the qni-style open circular dot', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
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

  const ringStrokeCount = signaturePoints
    .filter(({ name }) => name.startsWith('ring-'))
    .filter(({ name }) => isAntiControlStroke(pixels[name])).length
  expect({
    centerOpen: isAntiControlStroke(pixels.center),
    ringVisible: ringStrokeCount >= 8,
    outside: ['outside-left', 'outside-right'].map((name) => isAntiControlStroke(pixels[name])),
  }).toEqual({ centerOpen: false, ringVisible: true, outside: [false, false] })
})

test('control and anti-control have matching outer diameters', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
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

  expect({
    widthAligned: Math.abs(bounds.control.width - bounds.antiControl.width) <= 1,
    heightAligned: Math.abs(bounds.control.height - bounds.antiControl.height) <= 1,
  }).toEqual({ widthAligned: true, heightAligned: true })
})
