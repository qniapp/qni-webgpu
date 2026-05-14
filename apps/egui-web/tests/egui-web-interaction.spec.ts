import { expect, test } from '@playwright/test'
import {
  assertDragPreviewAboveOverlay,
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

const EXEC_MODE_LOCAL_FILL: CanvasPixel = [111, 110, 105, 255] // Flexoki tx-2 #6F6E69
const EXEC_MODE_GPU_FILL: CanvasPixel = [32, 94, 166, 255] // Flexoki blue-600 #205EA6

const readCircuitColsFromHash = (url: string): unknown[] => {
  const hash = new URL(url).hash.slice(1)
  if (!hash) {
    return []
  }
  return JSON.parse(decodeURIComponent(hash)).cols
}

const execModeProbePoints = (cssWidth: number): PixelSamplePoint[] => [
  { name: 'local', x: cssWidth - 120, y: 24 },
  { name: 'gpu', x: cssWidth - 64, y: 24 },
]

const RUN_GPU_BUTTON_POINT: Point = { x: 147, y: 18 }
const TEST_REM = 32
const TEST_GATE_SIZE = TEST_REM
const TEST_PALETTE_ROW_Y = 2.5 * TEST_REM
const TEST_PALETTE_ROW_GAP = 8
const TEST_PALETTE_PADDING_Y = 20
const TEST_PALETTE_CIRCUIT_GAP = 48
const TEST_CIRCUIT_LINE_Y =
  TEST_PALETTE_ROW_Y +
  TEST_GATE_SIZE * 2 +
  TEST_PALETTE_ROW_GAP +
  TEST_PALETTE_PADDING_Y +
  TEST_PALETTE_CIRCUIT_GAP +
  TEST_GATE_SIZE / 2
const TEST_CIRCUIT_LINE_GAP = 1.5 * TEST_REM

test('execution mode toggle switches visually without recomputing state', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const points = execModeProbePoints(cssWidth)

  const expectLocal = async () => {
    await expect
      .poll(async () => {
        const pixels = await sampleCanvasPixels(page, canvas, points)
        return pixelRgbDistance(pixels.local, EXEC_MODE_LOCAL_FILL)
      })
      .toBeLessThan(36)
  }
  const expectGpu = async () => {
    await expect
      .poll(async () => {
        const pixels = await sampleCanvasPixels(page, canvas, points)
        return pixelRgbDistance(pixels.gpu, EXEC_MODE_GPU_FILL)
      })
      .toBeLessThan(36)
  }

  const initialState = await readStateVector(page)
  await expectLocal()

  await page.mouse.click((box?.x ?? 0) + points[1].x, (box?.y ?? 0) + points[1].y)
  await expectGpu()
  expect(await readStateVector(page)).toEqual(initialState)

  await page.mouse.click((box?.x ?? 0) + points[0].x, (box?.y ?? 0) + points[0].y)
  await expectLocal()
  expect(await readStateVector(page)).toEqual(initialState)

  await page.mouse.click((box?.x ?? 0) + cssWidth / 2, (box?.y ?? 0) + 300)
  await page.keyboard.press('Tab')
  await page.keyboard.press('ArrowRight')
  await expectGpu()
  await page.keyboard.press('ArrowLeft')
  await expectLocal()
  await page.keyboard.press('Enter')
  await expectGpu()
  await page.keyboard.press('Space')
  await expectLocal()
  expect(await readStateVector(page)).toEqual(initialState)
})

test('empty hash checkpoint overrides a stale qni path payload on load', async ({ page }) => {
  const pathPayload = encodeURIComponent(JSON.stringify({ cols: [['X']] }))
  const emptyHash = encodeURIComponent(JSON.stringify({ cols: [] }))
  await page.goto(`/${pathPayload}#${emptyHash}`)

  await waitForStartupReady(page, { waitForStateVector: true })

  expect(readCircuitColsFromHash(page.url())).toEqual([])
  await waitForStateVectorApprox(page, [1, 0, 0, 0])
})

test('Run GPU refreshes the state-vector panel for small GPU-mode circuits', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const points = execModeProbePoints(cssWidth)
  const initialState = await readStateVector(page)

  await page.mouse.click((box?.x ?? 0) + points[1].x, (box?.y ?? 0) + points[1].y)
  await expect
    .poll(async () => {
      const pixels = await sampleCanvasPixels(page, canvas, points)
      return pixelRgbDistance(pixels.gpu, EXEC_MODE_GPU_FILL)
    })
    .toBeLessThan(36)

  await page.evaluate(() => {
    ;(window as any).__qniRunQiskitBackend = async (payloadJson: string) => {
      const payload = JSON.parse(payloadJson)
      ;(window as any).__qniLastQiskitRequest = payload
      const result = {
        status: 'completed',
        runner: 'test',
        qubits: payload.qubits,
        shots: payload.shots,
        histogram: { '0': 512, '1': 512 },
        truncated: false,
      }
      ;(window as any).__qniLastQiskitResult = result
      return result
    }
  })

  const REM = 32
  const GATE_SIZE = 1 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2.5 * REM
  const PALETTE_ROW_GAP = 8
  const PALETTE_PADDING_Y = 20
  const PALETTE_CIRCUIT_GAP = 48
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y =
    PALETTE_ROW_Y + GATE_SIZE * 2 + PALETTE_ROW_GAP + PALETTE_PADDING_Y + PALETTE_CIRCUIT_GAP + GATE_SIZE / 2

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  await dragPointer(page, hSource, { x: targetX, y: LINE_Y })

  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['H']])
  expect(await readStateVector(page)).toEqual(initialState)

  await page.mouse.click((box?.x ?? 0) + RUN_GPU_BUTTON_POINT.x, (box?.y ?? 0) + RUN_GPU_BUTTON_POINT.y)
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, Math.SQRT1_2, 0])

  await expect.poll(async () => page.evaluate(() => (window as any).__qniLastQiskitResult?.status))
    .toBe('completed')
})

test('toolbar undo and redo restore committed circuit history', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2.5 * REM
  const PALETTE_ROW_GAP = 8
  const PALETTE_PADDING_Y = 20
  const PALETTE_CIRCUIT_GAP = 48
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y =
    PALETTE_ROW_Y + GATE_SIZE * 2 + PALETTE_ROW_GAP + PALETTE_PADDING_Y + PALETTE_CIRCUIT_GAP + GATE_SIZE / 2

  const xSource = getPaletteGateCenter(cssWidth, 1)
  const hSource = getPaletteGateCenter(cssWidth, 0)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const clickToolbar = async (x: number): Promise<void> => {
    await page.mouse.click((box?.x ?? 0) + x, (box?.y ?? 0) + 18)
  }

  await dragPointer(page, xSource, { x: targetX, y: LINE_Y })
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['X']])
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await dragPointer(page, hSource, { x: targetX2, y: LINE_Y })
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['X'], ['H']])
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, -Math.SQRT1_2, 0])

  await clickToolbar(26)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['X']])
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await clickToolbar(26)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([])
  await waitForStateVectorApprox(page, [1, 0, 0, 0])

  await clickToolbar(62)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['X']])
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await clickToolbar(62)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['X'], ['H']])
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, -Math.SQRT1_2, 0])

  await clickToolbar(98)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([])
  await waitForStateVectorApprox(page, [1, 0, 0, 0])

  await clickToolbar(26)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['X'], ['H']])
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, -Math.SQRT1_2, 0])

  await clickToolbar(62)
  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([])
  await waitForStateVectorApprox(page, [1, 0, 0, 0])
})

test('Local mode refuses a 17th qubit drop', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 1200 })
  const col0: Array<string | number> = Array(16).fill(1)
  col0[15] = 'X'
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [col0] })))

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const source = getPaletteGateCenter(cssWidth, 0)
  const targetY17 = TEST_CIRCUIT_LINE_Y + 16 * TEST_CIRCUIT_LINE_GAP
  await dragPointer(page, source, { x: 180, y: targetY17 }, 8, true)
  await page.waitForTimeout(100)

  const cols = readCircuitColsFromHash(page.url()) as unknown[][]
  expect(cols.every((col) => col.length <= 16)).toBe(true)
  expect(await readStateVector(page)).toHaveLength(131072)
})

test('GPU mode accepts a 17th qubit drop', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 1200 })
  const col0: Array<string | number> = Array(16).fill(1)
  col0[15] = 'X'
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [col0] })))

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const points = execModeProbePoints(cssWidth)

  await page.mouse.click((box?.x ?? 0) + points[1].x, (box?.y ?? 0) + points[1].y)
  await expect
    .poll(async () => {
      const pixels = await sampleCanvasPixels(page, canvas, points)
      return pixelRgbDistance(pixels.gpu, EXEC_MODE_GPU_FILL)
    })
    .toBeLessThan(36)

  const source = getPaletteGateCenter(cssWidth, 0)
  const targetY17 = TEST_CIRCUIT_LINE_Y + 16 * TEST_CIRCUIT_LINE_GAP
  await dragPointer(page, source, { x: 180, y: targetY17 }, 8, true)
  await page.waitForTimeout(100)

  const cols = readCircuitColsFromHash(page.url()) as unknown[][]
  expect(cols.some((col) => col.length === 17)).toBe(true)
})

test('17-qubit GPU circuit keeps Local mode unavailable', async ({ page }) => {
  const col0: Array<string | number> = Array(17).fill(1)
  col0[16] = 'X'
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [col0] })))

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const points = execModeProbePoints(cssWidth)
  const expectGpu = async () => {
    await expect
      .poll(async () => {
        const pixels = await sampleCanvasPixels(page, canvas, points)
        return pixelRgbDistance(pixels.gpu, EXEC_MODE_GPU_FILL)
      })
      .toBeLessThan(36)
  }

  await expectGpu()
  const cols = readCircuitColsFromHash(page.url()) as unknown[][]
  expect(cols[0]).toHaveLength(17)

  await page.mouse.click((box?.x ?? 0) + points[0].x, (box?.y ?? 0) + points[0].y)
  await expectGpu()

  await page.keyboard.press('Tab')
  await page.keyboard.press('ArrowLeft')
  await expectGpu()
})

test('state panel hover does not drive circuit step preview', async ({ page }) => {
  const col0: Array<string | number> = Array(8).fill(1)
  col0[7] = 'X'
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [col0] })))

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const EGUI_PANEL_MARGIN = 8
  const slotCenter = (column: number) =>
    EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING * column
  const stepLineX = (column: number) => slotCenter(column) + SLOT_SPACING * 0.5
  const hoveredColumn = 2
  const probeY = 480
  const probePoints: PixelSamplePoint[] = [
    { name: 'line', x: stepLineX(hoveredColumn), y: probeY },
    { name: 'background', x: stepLineX(hoveredColumn) + 8, y: probeY },
  ]
  const stepLineContrast = async (): Promise<number> => {
    const pixels = await sampleCanvasPixels(page, canvas, probePoints)
    return pixelRgbDistance(pixels.line, pixels.background)
  }

  await page.mouse.move(
    (box?.x ?? 0) + slotCenter(hoveredColumn),
    (box?.y ?? 0) + probeY
  )
  await expect.poll(stepLineContrast).toBeGreaterThan(50)

  await page.mouse.move(
    (box?.x ?? 0) + slotCenter(hoveredColumn),
    (box?.y ?? 0) + 560
  )
  await expect.poll(stepLineContrast).toBeLessThan(25)
})

test('state cell popup hides while dragging over the state panel', async ({ page }) => {
  await page.goto('/')

  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? 1000
  const cssHeight = box?.height ?? 800

  const EGUI_PANEL_MARGIN = 8
  const STATE_PANEL_WIDTH = 560
  const STATE_VIEWPORT_HEIGHT = 160
  const STATE_HANDLE_HEIGHT = 32
  const STATE_BOTTOM_MARGIN = 64
  const STATE_CELL_SIZE = 64
  const STATE_CELL_GAP = 3
  const innerWidth = cssWidth - EGUI_PANEL_MARGIN * 2
  const innerHeight = cssHeight - EGUI_PANEL_MARGIN * 2
  const stateRectMinX = EGUI_PANEL_MARGIN + innerWidth / 2 - STATE_PANEL_WIDTH / 2
  const stateRectMinY =
    EGUI_PANEL_MARGIN + innerHeight - STATE_BOTTOM_MARGIN - STATE_VIEWPORT_HEIGHT - STATE_HANDLE_HEIGHT
  const viewportMinY = stateRectMinY + STATE_HANDLE_HEIGHT
  const gridWidth = STATE_CELL_SIZE * 2 + STATE_CELL_GAP
  const cellCenter = {
    x: stateRectMinX + (STATE_PANEL_WIDTH - gridWidth) / 2 + STATE_CELL_SIZE / 2,
    y: viewportMinY + (STATE_VIEWPORT_HEIGHT - STATE_CELL_SIZE) / 2 + STATE_CELL_SIZE / 2,
  }
  const popupHeight = 108
  const popupTop = cellCenter.y - STATE_CELL_SIZE / 2 - 4 - 8 - popupHeight
  const popupFill = { name: 'popupFill', x: cellCenter.x, y: popupTop + 16 }
  const nearbyBackground = { name: 'nearbyBackground', x: cellCenter.x, y: popupTop - 12 }
  const popupContrast = async (): Promise<number> => {
    const pixels = await sampleCanvasPixels(page, canvas, [popupFill, nearbyBackground])
    return pixelRgbDistance(pixels.popupFill, pixels.nearbyBackground)
  }

  const POPUP_WIDTH = 296
  const POPUP_PAD_X = 16
  const POPUP_HEADER_TEXT_H = 20
  const POPUP_HEADER_GAP = 8
  const POPUP_ICON_SIZE = 16
  const POPUP_ICON_TEXT_GAP = 8
  const POPUP_VALUE_X_OFFSET = POPUP_ICON_SIZE + POPUP_ICON_TEXT_GAP + 96
  const popupValueAnchor = {
    x: cellCenter.x - POPUP_WIDTH / 2 + POPUP_PAD_X + POPUP_VALUE_X_OFFSET,
    y: popupTop + 12 + POPUP_HEADER_TEXT_H + POPUP_HEADER_GAP - 2,
  }
  const popupValuePoints: PixelSamplePoint[] = []
  for (let y = popupValueAnchor.y; y <= popupValueAnchor.y + 56; y += 4) {
    for (let x = popupValueAnchor.x; x <= popupValueAnchor.x + 148; x += 6) {
      popupValuePoints.push({ name: `popupValue-${popupValuePoints.length}`, x, y })
    }
  }
  const popupValueInkCount = async (): Promise<number> => {
    const pixels = await sampleCanvasPixels(page, canvas, [popupFill, ...popupValuePoints])
    return popupValuePoints.filter(
      (point) => pixelRgbDistance(pixels[point.name], pixels.popupFill) > 80
    ).length
  }

  await page.mouse.move((box?.x ?? 0) + cellCenter.x, (box?.y ?? 0) + cellCenter.y)
  await expect.poll(popupContrast).toBeGreaterThan(20)
  await expect.poll(popupValueInkCount).toBeGreaterThan(10)

  const hSource = getPaletteGateCenter(cssWidth, 0)
  await dragPointer(page, hSource, cellCenter, 8, false)
  await expect.poll(popupContrast).toBeLessThan(15)
  await page.mouse.up()
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
  const QUBIT_LABEL_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2.5 * REM
  const PALETTE_ROW_GAP = 8
  const PALETTE_PADDING_Y = 20
  const PALETTE_CIRCUIT_GAP = 48
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y =
    PALETTE_ROW_Y + GATE_SIZE * 2 + PALETTE_ROW_GAP + PALETTE_PADDING_Y + PALETTE_CIRCUIT_GAP + GATE_SIZE / 2
  const LINE_GAP = 1.5 * REM

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  const targetY2 = LINE_Y + 2 * LINE_GAP

  await dragPointer(page, hSource, { x: targetX, y: targetY0 })

  await dragPointer(page, hSource, { x: targetX, y: targetY1 })
  await waitForStateVectorLength(page, 8)

  const colsBeforeDrag = readCircuitColsFromHash(page.url())
  await dragPointer(page, { x: targetX, y: targetY0 }, { x: targetX, y: targetY2 }, 6, false)

  const lengthDuringDrag = (await readStateVector(page)).length
  expect(lengthDuringDrag).toBe(8)
  expect(readCircuitColsFromHash(page.url())).toEqual(colsBeforeDrag)

  await releasePointer(page, { x: targetX, y: targetY2 })

  await waitForStateVectorLength(page, 16)
})

test('dropping between existing columns inserts a new column', async ({ page }) => {
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
  const QUBIT_LABEL_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2.5 * REM
  const PALETTE_ROW_GAP = 8
  const PALETTE_PADDING_Y = 20
  const PALETTE_CIRCUIT_GAP = 48
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y =
    PALETTE_ROW_Y + GATE_SIZE * 2 + PALETTE_ROW_GAP + PALETTE_PADDING_Y + PALETTE_CIRCUIT_GAP + GATE_SIZE / 2

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const ySource = getPaletteGateCenter(cssWidth, 2)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const insertX = targetX + SLOT_SPACING / 2

  await dragPointer(page, hSource, { x: targetX, y: LINE_Y })
  await dragPointer(page, xSource, { x: targetX2, y: LINE_Y })

  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([['H'], ['X']])

  await dragPointer(page, ySource, { x: insertX, y: LINE_Y })

  await expect.poll(async () => readCircuitColsFromHash(page.url())).toEqual([
    ['H'],
    ['Y'],
    ['X'],
  ])
})
