const { test, expect } = require('@playwright/test')
const { chromium } = require('playwright')

const { assertDragPreviewAboveOverlay } = require('../features/support/assertions.cjs')
const {
  dragPointer,
  getDragPreviewAboveStatePanelProbe,
  readEguiError,
  readStateVector,
  releasePointer,
  sampleCanvasPixels,
  waitForAppReady,
  waitForCanvasContent,
  waitForStartupReady,
} = require('../features/support/egui-helpers.cjs')

const waitForStateVectorLength = async (page, length, timeout = 5000) => {
  await expect
    .poll(async () => (await readStateVector(page)).length, { timeout })
    .toBe(length)
}

const waitForStateVectorApprox = async (page, expected, timeout = 5000, tolerance = 1e-3) => {
  await expect
    .poll(async () => {
      const actual = await readStateVector(page)
      if (actual.length !== expected.length) {
        return false
      }
      return expected.every((value, index) => Math.abs(actual[index] - value) < tolerance)
    }, { timeout })
    .toBe(true)
}

test('egui webgpu canvas renders content', async ({ page }) => {
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
    path: '/tmp/qni-egui-webgpu-initial.png',
  })
  expect(initialRender.nonBackground).toBeGreaterThanOrEqual(40)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2
  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y
  const targetX2 = targetX + SLOT_SPACING
  const targetY2 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY })

  const expected = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY })

  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX2, y: targetY2 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)

  const afterRender = await waitForCanvasContent(page, canvas, {
    path: '/tmp/qni-egui-webgpu-after.png',
  })
  expect(afterRender.nonBackground).toBeGreaterThanOrEqual(40)
  await canvas.screenshot({ path: '/tmp/qni-egui-webgpu.png' })
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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  const expectedAfterQ0 = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedAfterQ0)

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })

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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  const targetY2 = LINE_Y + 2 * LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })
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
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const palettePadding = REM
  const paletteRect = {
    x: paletteStartX - palettePadding,
    y: PALETTE_ROW_Y - palettePadding,
    width: paletteWidth + palettePadding * 2,
    height: PALETTE_SIZE + palettePadding * 2,
  }
  const sourceX = paletteStartX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const dragTarget = { x: sourceX + 80, y: sourceY + 80 }
  const panelPoints = [
    { name: 'corner', x: paletteRect.x + 2, y: paletteRect.y + 2 },
    { name: 'fill', x: paletteRect.x + 24, y: paletteRect.y + 24 },
    { name: 'shadow', x: paletteRect.x + paletteRect.width / 2, y: paletteRect.y + paletteRect.height + 10 },
    { name: 'background', x: paletteRect.x - 20, y: paletteRect.y + paletteRect.height + 10 },
  ]

  const beforeDrag = await sampleCanvasPixels(page, canvas, panelPoints)

  await dragPointer(page, { x: sourceX, y: sourceY }, dragTarget, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, panelPoints)

  for (const name of ['corner', 'fill']) {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    expect(diff).toBeLessThan(40)
  }

  const cornerBrightness = duringDrag.corner[0] + duringDrag.corner[1] + duringDrag.corner[2]
  const fillBrightness = duringDrag.fill[0] + duringDrag.fill[1] + duringDrag.fill[2]
  expect(Math.abs(cornerBrightness - fillBrightness)).toBeGreaterThan(10)

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

  const REM = 32
  const PALETTE_SIZE = REM
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const dragSource = { x: paletteStartX + PALETTE_SIZE / 2, y: PALETTE_ROW_Y + PALETTE_SIZE / 2 }
  const dragTarget = { x: dragSource.x + 80, y: dragSource.y + 80 }
  const controlIndex = 1
  const controlRect = {
    x: paletteStartX + controlIndex * (PALETTE_SIZE + PALETTE_GAP),
    y: PALETTE_ROW_Y,
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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const sourceX = paletteStartX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const dragTarget = {
    x: paletteStartX - PALETTE_SIZE / 2,
    y: sourceY,
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

  await dragPointer(page, { x: sourceX, y: sourceY }, dragTarget, 6, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [fillPoint])

  const before = beforeDrag.fill
  const during = duringDrag.fill
  const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
  expect(diff).toBeGreaterThan(120)
  expect(during[1]).toBeGreaterThan(during[0] + 40)

  await page.mouse.up()
})

test('dragged palette gate stays above the state panel overlay', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()

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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const sourceX = startX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const dragTarget = { x: sourceX + 80, y: sourceY + 80 }
  const dragRect = {
    x: dragTarget.x - GATE_SIZE / 2,
    y: dragTarget.y - GATE_SIZE / 2,
  }

  await dragPointer(page, { x: sourceX, y: sourceY }, dragTarget, 6, false)
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

test('dragged x gate keeps the same visual as after drop', async ({ page }) => {
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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const gateIndex = 2
  const sourceX = startX + gateIndex * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const targetRect = {
    x: targetCenter.x - GATE_SIZE / 2,
    y: targetCenter.y - GATE_SIZE / 2,
  }
  const signaturePoints = [
    { name: 'center', x: targetRect.x + GATE_SIZE / 2, y: targetRect.y + GATE_SIZE / 2 },
    { name: 'top', x: targetRect.x + GATE_SIZE / 2, y: targetRect.y + 6 },
    { name: 'bottom', x: targetRect.x + GATE_SIZE / 2, y: targetRect.y + GATE_SIZE - 6 },
    { name: 'left', x: targetRect.x + 6, y: targetRect.y + GATE_SIZE / 2 },
    { name: 'right', x: targetRect.x + GATE_SIZE - 6, y: targetRect.y + GATE_SIZE / 2 },
  ]

  await dragPointer(page, { x: sourceX, y: sourceY }, targetCenter, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await releasePointer(page, targetCenter)
  await page.waitForTimeout(50)
  const afterDrop = await sampleCanvasPixels(page, canvas, signaturePoints)

  for (const name of Object.keys(duringDrag)) {
    const during = duringDrag[name]
    const after = afterDrop[name]
    const diff = Math.abs(during[0] - after[0]) + Math.abs(during[1] - after[1]) + Math.abs(during[2] - after[2])
    expect(diff).toBeLessThan(60)
  }
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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const sqrtXGateCenter = { x: paletteCenterX(5), y: sourceY }
  const hGateCenter = { x: paletteCenterX(0), y: sourceY }
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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })

  const expectedAfterH = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0, 0, 0, 0, 0]
  await waitForStateVectorApprox(page, expectedAfterH)

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY1 })

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX2, y: targetY0 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)
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
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetX3 = targetX2 + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY0 })

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX3, y: targetY1 })

  const expected = [0, 0, 1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)
})

test('default chromium shows a visible WebGPU error instead of a blank page', async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH || chromium.executablePath(),
    args: ['--disable-gpu', '--disable-software-rasterizer'],
  })

  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } })
    await page.goto('http://127.0.0.1:4174/', { waitUntil: 'load' })
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
