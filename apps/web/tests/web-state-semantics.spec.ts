import { test } from '@playwright/test'
import {
  dragPointer,
  getCircuitOriginY,
  getPaletteGateCenter,
  UI_CONSTANTS,
  waitForStartupReady,
  waitForStateVectorApprox,
} from './support/web-spec-helpers'

test('H on q0 and q1 yields uniform superposition', async ({ page }) => {
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
  const LINE_Y = await getCircuitOriginY(page)
  const LINE_GAP = UI_CONSTANTS.LINE_GAP

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

test('CNOT with control on q1 yields bell state', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)
  const LINE_GAP = UI_CONSTANTS.LINE_GAP

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const controlSource = getPaletteGateCenter(cssWidth, 14)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, hSource, { x: targetX, y: targetY1 })

  const expectedAfterH = [1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0, 0, 0]
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
  const LINE_Y = await getCircuitOriginY(page)
  const LINE_GAP = UI_CONSTANTS.LINE_GAP

  const antiControlSource = getPaletteGateCenter(cssWidth, 15)
  const xSource = getPaletteGateCenter(cssWidth, 1)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, antiControlSource, { x: targetX, y: targetY0 })
  await waitForStateVectorApprox(page, [1, 0, 0, 0])

  await dragPointer(page, xSource, { x: targetX, y: targetY1 })
  await waitForStateVectorApprox(page, [0, 0, 0, 0, 1, 0, 0, 0])
})

test('anti-control does not apply when the control wire is one', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)
  const LINE_GAP = UI_CONSTANTS.LINE_GAP

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
  await waitForStateVectorApprox(page, [0, 0, 1, 0, 0, 0, 0, 0])
})

test('Control suppresses QFT when the control wire is zero', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['•', 'QFT2']] })))

  await waitForStartupReady(page, { waitForStateVector: true })

  const expected = Array(16).fill(0)
  expected[0] = 1
  await waitForStateVectorApprox(page, expected)
})

test('Control lets QFT run when the control wire is one', async ({ page }) => {
  await page.goto(
    '/#' +
      encodeURIComponent(
        JSON.stringify({ cols: [['|1>', 1, 1], ['•', 'QFT2', 1]] }),
      ),
  )

  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(226, await getCircuitOriginY(page) - 32)
  await page.mouse.move(1000, 450)

  const expected = Array(16).fill(0)
  expected[2] = 0.5
  expected[6] = 0.5
  expected[10] = 0.5
  expected[14] = 0.5
  await waitForStateVectorApprox(page, expected)
})

test('Control does not affect gates in other columns', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)
  const LINE_GAP = UI_CONSTANTS.LINE_GAP

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

  const expected = [0, 0, 0, 0, 1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)
})

test('|0> resets a flipped qubit back to |0>', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)

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
  const LINE_Y = await getCircuitOriginY(page)

  const write1Source = getPaletteGateCenter(cssWidth, 18)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y

  await dragPointer(page, write1Source, { x: targetX, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])
})

test('|0> after H resets the superposition to |0>', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const write0Source = getPaletteGateCenter(cssWidth, 17)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, hSource, { x: targetX, y: targetY })
  const superposition = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, superposition)

  await dragPointer(page, write0Source, { x: targetX2, y: targetY })
  await waitForStateVectorApprox(page, [1, 0, 0, 0])
})

test('Bloch display does not alter the state vector', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const blochSource = getPaletteGateCenter(cssWidth, 16)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, hSource, { x: targetX, y: targetY })
  const superposition = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, superposition)

  await dragPointer(page, blochSource, { x: targetX2, y: targetY })
  await waitForStateVectorApprox(page, superposition)
})

test('Measurement after X collapses the qubit to |1>', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)

  const xSource = getPaletteGateCenter(cssWidth, 1)
  const measureSource = getPaletteGateCenter(cssWidth, 19)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, xSource, { x: targetX, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])

  await dragPointer(page, measureSource, { x: targetX2, y: targetY })
  await waitForStateVectorApprox(page, [0, 0, 1, 0])
})

test('Spacer is a NOP and does not alter the state vector', async ({ page }) => {
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
  const SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 0.5 * REM
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = await getCircuitOriginY(page)

  const hSource = getPaletteGateCenter(cssWidth, 0)
  const spacerSource = getPaletteGateCenter(cssWidth, 21)
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY = LINE_Y

  await dragPointer(page, hSource, { x: targetX, y: targetY })
  const superposition = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, superposition)

  await dragPointer(page, spacerSource, { x: targetX2, y: targetY })
  await waitForStateVectorApprox(page, superposition)
})

test('standalone app shows the final step of a loaded circuit', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent('{"cols":[["|0>","|0>"],["H"],["•","X"]]}')}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await waitForStateVectorApprox(page, [Math.SQRT1_2, 0, 0, 0, 0, 0, Math.SQRT1_2, 0])
})
