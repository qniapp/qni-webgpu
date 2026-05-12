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
