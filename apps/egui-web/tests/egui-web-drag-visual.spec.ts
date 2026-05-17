import { expect, test } from '@playwright/test'
import {
  dragPreviewAboveOverlayIssue,
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

test('dragged palette gate stays visible above the palette panel', async ({ page }) => {
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
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
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
  expect({ diffVisible: diff > 120, usesDragFill: isDragPreviewFill(during) }).toEqual({
    diffVisible: true,
    usesDragFill: true,
  })

  await page.mouse.up()
})

test('dragged palette gate stays above the state panel overlay', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  if (!box) {
    throw new Error('canvas bounding box should be available')
  }

  const { source, handleCenter, dragFillPoint, sourceFillPoint } =
    getDragPreviewAboveStatePanelProbe(box.width, box.height)
  const beforeDrag = await sampleCanvasPixels(page, canvas, [dragFillPoint, sourceFillPoint])

  await dragPointer(page, source, handleCenter, 8, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [dragFillPoint])

  const issue = dragPreviewAboveOverlayIssue({
    before: beforeDrag.fill,
    during: duringDrag.fill,
    source: beforeDrag.sourceFill,
  })

  await page.mouse.up()
  expect(issue).toBeNull()
})

test('dragged palette gate keeps rounded corners', async ({ page }) => {
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
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
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
  await canvas.waitFor({ state: 'visible' })
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = UI_CONSTANTS.LINE_Y
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
  expect({ draggedUsesPurple: duringPurpleCount > 20, droppedReturnsGreen: afterGreenCount > 20 }).toEqual({
    draggedUsesPurple: true,
    droppedReturnsGreen: true,
  })
})

test('x gate uses a circular body in palette, circuit, and drag preview', async ({ page }, testInfo) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })
  await waitForCanvasContent(page, canvas)
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = UI_CONSTANTS.LINE_Y
  const xGateCenter = getPaletteGateCenter(cssWidth, 1)
  const placedXCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const dragXCenter = { x: placedXCenter.x + 80, y: placedXCenter.y + 40 }
  const readCircularBodySignature = async (center: Point): Promise<CircularBodySignature> => {
    const screenshot = await canvas.screenshot({ type: 'png' })
    const base64 = screenshot.toString('base64')
    const canvasBox = await canvas.boundingBox()
    if (!canvasBox) {
    throw new Error('expected egui canvas to be measurable')
  }

    return page.evaluate<
      CircularBodySignature,
      { base64: string; center: Point; cssWidth: number; cssHeight: number; gateSize: number }
    >(
      async ({ base64, center, cssWidth, cssHeight, gateSize }) => {
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
        const searchRadius = Math.ceil(gateSize * 0.9)
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
        const insideRadius = Math.floor(gateSize / 2) - 3
        const outsideDiagonal = Math.floor(gateSize / 2) - 3
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
        gateSize: GATE_SIZE,
      }
    )
  }
  const readCircularBodyCheck = async (center: Point) => {
    const signature = await readCircularBodySignature(center)
    const edgeNames = ['top', 'bottom', 'left', 'right']
    const cornerNames = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight']
    const filledEdges = edgeNames.filter((name) => isGateBodyFill(signature.samples[name])).length
    const filledCorners = cornerNames.filter((name) => isGateBodyFill(signature.samples[name])).length
    return {
      countOk: signature.count > 900,
      widthOk: signature.width > 32,
      heightOk: signature.height > 32,
      filledEdges,
      filledCorners,
    }
  }

  const palette = await readCircularBodyCheck({ x: xGateCenter.x, y: xGateCenter.y + 8 })

  await dragPointer(page, xGateCenter, placedXCenter)
  await page.waitForTimeout(50)
  const placed = await readCircularBodyCheck({ x: placedXCenter.x + 8, y: placedXCenter.y + 8 })

  await dragPointer(page, placedXCenter, dragXCenter, 6, false)
  await page.waitForTimeout(50)
  const dragPreview = await readCircularBodyCheck({ x: dragXCenter.x - GATE_SIZE / 4, y: dragXCenter.y + GATE_SIZE / 2 + 2 })
  await canvas.screenshot({ path: testInfo.outputPath('x-gate-circular-body.png') })

  expect({ palette, placed, dragPreview }).toEqual({
    palette: { countOk: true, widthOk: true, heightOk: true, filledEdges: 4, filledCorners: 0 },
    placed: { countOk: true, widthOk: true, heightOk: true, filledEdges: 4, filledCorners: 0 },
    dragPreview: { countOk: true, widthOk: true, heightOk: true, filledEdges: 4, filledCorners: 0 },
  })

  await page.mouse.up()
})

test('drag preview preserves resized QFT span', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['QFT3']] }))}`)

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const REM = 32
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
  const LINE_GAP = UI_CONSTANTS.LINE_GAP
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = UI_CONSTANTS.LINE_Y
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING

  const placedCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const dragTarget = { x: placedCenter.x + SLOT_SPACING, y: placedCenter.y }
  const lowerSpanFillPoint = {
    name: 'lowerSpanFill',
    x: dragTarget.x,
    y: dragTarget.y + 2 * LINE_GAP,
  }

  await dragPointer(page, placedCenter, dragTarget, 6, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [lowerSpanFillPoint])
  expect(isDragPreviewFill(duringDrag.lowerSpanFill)).toBe(true)

  await page.mouse.up()
})

test('placed circuit gate keeps its visual while dragging another gate', async ({ page }) => {
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
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = UI_CONSTANTS.LINE_Y

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
