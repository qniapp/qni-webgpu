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
  UI_CONSTANTS,
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

test('SVG SDF gate labels keep palette and circuit glyph weight aligned', async ({ page }) => {
  await page.setViewportSize({ width: 1001, height: 800 })
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['H'], ['X'], ['Y'], ['Z'], ['X^½'], ['S'], ['S†'], ['T'], ['T†'], ['P'], ['Rx'], ['Ry'], ['Rz'], ['QFT2'], ['QFT†2']] })))

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }

  const EGUI_PANEL_MARGIN = 8
  const gateSize = UI_CONSTANTS.GATE_SIZE
  const labels = [
    { name: 'H', paletteIndex: 0, circuitIndex: 0, circuitYOffset: 0 },
    { name: 'X', paletteIndex: 1, circuitIndex: 1, circuitYOffset: 0 },
    { name: 'Y', paletteIndex: 2, circuitIndex: 2, circuitYOffset: 0 },
    { name: 'Z', paletteIndex: 3, circuitIndex: 3, circuitYOffset: 0 },
    { name: 'SqrtX', paletteIndex: 4, circuitIndex: 4, circuitYOffset: 0 },
    { name: 'S', paletteIndex: 5, circuitIndex: 5, circuitYOffset: 0 },
    { name: 'SDagger', paletteIndex: 6, circuitIndex: 6, circuitYOffset: 0 },
    { name: 'T', paletteIndex: 7, circuitIndex: 7, circuitYOffset: 0 },
    { name: 'TDagger', paletteIndex: 8, circuitIndex: 8, circuitYOffset: 0 },
    { name: 'P', paletteIndex: 9, circuitIndex: 9, circuitYOffset: 0 },
    { name: 'RX', paletteIndex: 10, circuitIndex: 10, circuitYOffset: 0 },
    { name: 'RY', paletteIndex: 11, circuitIndex: 11, circuitYOffset: 0 },
    { name: 'RZ', paletteIndex: 12, circuitIndex: 12, circuitYOffset: 0 },
    { name: 'QFT', paletteIndex: 22, circuitIndex: 13, circuitYOffset: UI_CONSTANTS.LINE_GAP / 2 },
    { name: 'QFTDagger', paletteIndex: 23, circuitIndex: 14, circuitYOffset: UI_CONSTANTS.LINE_GAP / 2 },
  ] as const
  const centers = Object.fromEntries(labels.flatMap(({ name, paletteIndex, circuitIndex, circuitYOffset }) => {
    const paletteCenter = getPaletteGateCenter(box.width, paletteIndex)
    return [
      [`palette${name}`, { x: paletteCenter.x, y: EGUI_PANEL_MARGIN + paletteCenter.y }],
      [`circuit${name}`, {
        x: EGUI_PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING * circuitIndex,
        y: EGUI_PANEL_MARGIN + UI_CONSTANTS.LINE_Y + circuitYOffset,
      }],
    ]
  }))
  const screenshot = await canvas.screenshot({ type: 'png' })
  const metrics = await page.evaluate<
    Record<string, { count: number; width: number; height: number }>,
    { base64: string; cssWidth: number; cssHeight: number; gateSize: number; centers: Record<string, Point> }
  >(async ({ base64, cssWidth, cssHeight, gateSize, centers }) => {
    const img = new Image()
    img.src = `data:image/png;base64,${base64}`
    await new Promise((resolve, reject) => {
      img.onload = () => resolve(null)
      img.onerror = () => reject(new Error('Failed to decode screenshot'))
    })

    const probe = document.createElement('canvas')
    probe.width = img.width
    probe.height = img.height
    const ctx = probe.getContext('2d')
    if (!ctx) {
      throw new Error('expected 2d context')
    }
    ctx.drawImage(img, 0, 0)
    const scaleX = img.width / cssWidth
    const scaleY = img.height / cssHeight
    const measure = ({ x: cx, y: cy }: Point) => {
      let minX = Number.POSITIVE_INFINITY
      let minY = Number.POSITIVE_INFINITY
      let maxX = Number.NEGATIVE_INFINITY
      let maxY = Number.NEGATIVE_INFINITY
      let count = 0
      const left = cx - gateSize / 2
      const top = cy - gateSize / 2
      for (let y = 4; y < gateSize - 4; y += 1) {
        for (let x = 4; x < gateSize - 4; x += 1) {
          const [r, g, b, a] = ctx.getImageData(Math.floor((left + x) * scaleX), Math.floor((top + y) * scaleY), 1, 1).data
          const isLabelInk = a > 128 && r > 245 && g > 243 && b > 232
          if (isLabelInk) {
            count += 1
            minX = Math.min(minX, x)
            minY = Math.min(minY, y)
            maxX = Math.max(maxX, x)
            maxY = Math.max(maxY, y)
          }
        }
      }
      return { count, width: maxX - minX + 1, height: maxY - minY + 1 }
    }
    return Object.fromEntries(Object.entries(centers).map(([name, center]) => [name, measure(center)]))
  }, { base64: screenshot.toString('base64'), cssWidth: box.width, cssHeight: box.height, gateSize, centers })

  expect(metrics).toEqual({
    paletteH: { count: 28, width: 10, height: 14 },
    circuitH: { count: 28, width: 10, height: 14 },
    paletteX: { count: 4, width: 2, height: 2 },
    circuitX: { count: 4, width: 2, height: 2 },
    paletteY: { count: 14, width: 10, height: 8 },
    circuitY: { count: 14, width: 10, height: 8 },
    paletteZ: { count: 27, width: 8, height: 14 },
    circuitZ: { count: 27, width: 8, height: 14 },
    paletteSqrtX: { count: 38, width: 20, height: 16 },
    circuitSqrtX: { count: 38, width: 20, height: 16 },
    paletteS: { count: 29, width: 10, height: 14 },
    circuitS: { count: 29, width: 10, height: 14 },
    paletteSDagger: { count: 30, width: 16, height: 18 },
    circuitSDagger: { count: 30, width: 16, height: 18 },
    paletteT: { count: 10, width: 10, height: 1 },
    circuitT: { count: 10, width: 10, height: 1 },
    paletteTDagger: { count: 11, width: 16, height: 5 },
    circuitTDagger: { count: 11, width: 16, height: 5 },
    paletteP: { count: 32, width: 10, height: 14 },
    circuitP: { count: 32, width: 10, height: 14 },
    paletteRX: { count: 28, width: 16, height: 10 },
    circuitRX: { count: 28, width: 16, height: 10 },
    paletteRY: { count: 27, width: 16, height: 10 },
    circuitRY: { count: 27, width: 16, height: 10 },
    paletteRZ: { count: 43, width: 16, height: 10 },
    circuitRZ: { count: 43, width: 16, height: 10 },
    paletteQFT: { count: 36, width: 22, height: 10 },
    circuitQFT: { count: 36, width: 22, height: 10 },
    paletteQFTDagger: { count: 43, width: 23, height: 17 },
    circuitQFTDagger: { count: 43, width: 23, height: 17 },
  })
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
