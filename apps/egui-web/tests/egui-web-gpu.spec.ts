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

test('GPU circuit overlays stay optically anchored to measurement and Bloch bodies', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['X'], ['Measure'], ['Bloch']] })))

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  // Egui keeps an 8px panel margin inside the canvas; the interaction helpers
  // can be fuzzy because snap distance absorbs it, but visual pixel probes need
  // the actual painted position.
  const EGUI_PANEL_MARGIN = 8
  const slotCenter = (column: number) =>
    EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING * column
  const wireCenterY = EGUI_PANEL_MARGIN + LINE_Y

  const measureX = slotCenter(1)
  const blochX = slotCenter(2)
  const samples = await sampleCanvasPixels(page, canvas, [
    { name: 'measurement_gap_left', x: measureX - 20, y: wireCenterY },
    { name: 'measurement_wire_left', x: measureX - 24, y: wireCenterY },
    { name: 'measurement_digit_centered', x: measureX, y: wireCenterY },
    { name: 'bloch_tip_inside_sphere', x: blochX, y: wireCenterY + 12 },
    { name: 'bloch_tip_outside_sphere', x: blochX, y: wireCenterY + 16 },
  ])

  const isOutcomeBlue = ([r, g, b]: CanvasPixel): boolean => b > 130 && r < 140 && g < 190
  const isBlochRed = ([r, g, b]: CanvasPixel): boolean => r > 140 && g < 100 && b < 100
  const isCircuitBackground = ([r, g, b]: CanvasPixel): boolean => r > 240 && g > 240 && b > 240
  const isWireLine = ([r, g, b]: CanvasPixel): boolean =>
    Math.abs(r - 218) + Math.abs(g - 216) + Math.abs(b - 206) < 40

  expect(isCircuitBackground(samples.measurement_gap_left)).toBe(true)
  expect(isWireLine(samples.measurement_wire_left)).toBe(true)
  expect(isOutcomeBlue(samples.measurement_digit_centered)).toBe(true)
  expect(isBlochRed(samples.bloch_tip_inside_sphere)).toBe(true)
  expect(isBlochRed(samples.bloch_tip_outside_sphere)).toBe(false)
})

test('GPU circuit overlays stay anchored in tall scroll-area viewports', async ({ page }) => {
  const col0 = Array(16).fill(1)
  col0[0] = 'X'
  col0[15] = 'H'
  const col1 = Array(16).fill(1)
  col1[1] = 'Measure'
  const col2 = Array(16).fill(1)
  col2[0] = 'Measure'
  col2[2] = 'Bloch'
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [col0, col1, col2] })))

  await waitForStartupReady(page, { waitForStateVector: true })
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const EGUI_PANEL_MARGIN = 8
  const slotCenter = (column: number) =>
    EGUI_PANEL_MARGIN + LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING * column
  const wireCenterY = (wire: number) => EGUI_PANEL_MARGIN + LINE_Y + LINE_GAP * wire

  const overlayX = slotCenter(2)
  const samples = await sampleCanvasPixels(page, canvas, [
    { name: 'measurement_digit_too_high', x: overlayX, y: wireCenterY(0) - 12 },
    { name: 'measurement_digit_on_wire', x: overlayX, y: wireCenterY(0) },
    { name: 'bloch_tip_inside_sphere', x: overlayX, y: wireCenterY(2) - 14 },
    { name: 'bloch_tip_too_high', x: overlayX, y: wireCenterY(2) - 24 },
  ])

  const isOutcomeBlue = ([r, g, b]: CanvasPixel): boolean => b > 130 && r < 140 && g < 190
  const isBlochRed = ([r, g, b]: CanvasPixel): boolean => r > 140 && g < 100 && b < 100

  expect(isOutcomeBlue(samples.measurement_digit_too_high)).toBe(false)
  expect(isOutcomeBlue(samples.measurement_digit_on_wire)).toBe(true)
  expect(isBlochRed(samples.bloch_tip_inside_sphere)).toBe(true)
  expect(isBlochRed(samples.bloch_tip_too_high)).toBe(false)
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
