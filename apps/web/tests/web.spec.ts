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
} from './support/web-spec-helpers'

test('web canvas renders content', async ({ page }, testInfo) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  const gpuAvailable = await page.evaluate(() => Boolean(navigator.gpu))

  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const initialState = await readStateVector(page)

  const initialRender = await waitForCanvasContent(page, canvas, {
    path: testInfo.outputPath('qni-webgpu-initial.png'),
  })

  const REM = 32
  const GATE_SIZE = UI_CONSTANTS.GATE_SIZE
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = UI_CONSTANTS.LINE_Y
  const LINE_GAP = UI_CONSTANTS.LINE_GAP
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
    path: testInfo.outputPath('qni-webgpu-after.png'),
  })
  expect({
    gpuAvailable,
    initialState,
    initialRenderHasContent: initialRender.nonBackground >= 40,
    afterRenderHasContent: afterRender.nonBackground >= 40,
  }).toEqual({
    gpuAvailable: true,
    initialState: [1, 0, 0, 0],
    initialRenderHasContent: true,
    afterRenderHasContent: true,
  })
  await canvas.screenshot({ path: testInfo.outputPath('qni-webgpu.png') })
})
