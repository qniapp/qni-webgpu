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

const readCircuitColsFromHash = (url: string): unknown[] => {
  const hash = new URL(url).hash.slice(1)
  if (!hash) {
    return []
  }
  return JSON.parse(decodeURIComponent(hash)).cols
}

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

  await page.mouse.move((box?.x ?? 0) + cellCenter.x, (box?.y ?? 0) + cellCenter.y)
  await expect.poll(popupContrast).toBeGreaterThan(20)

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
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM

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
