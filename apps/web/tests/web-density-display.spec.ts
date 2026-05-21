import { expect, test } from '@playwright/test'
import {
  pixelRgbDistance,
  readDensityMatrixCell,
  sampleCanvasPixels,
  UI_CONSTANTS,
  waitForStartupReady,
  type DensityMatrixCell,
} from './support/web-spec-helpers'

type TestPage = Parameters<typeof readDensityMatrixCell>[0]

const EGUI_PANEL_MARGIN = 8
const DENSITY_CELL_FILL = [146, 191, 219, 255] // Flexoki blue-200 #92BFDB.
const DENSITY_CELL_BACKGROUND = [255, 252, 240, 255] // Flexoki bg #FFFCF0.
const DENSITY_MATRIX_BORDER = [218, 216, 206, 255] // Flexoki ui-2 #DAD8CE.

const rounded = (value: number): number => Math.round(value * 1_000) / 1_000

const readDensityCellUntilReady = async (
  page: TestPage,
  gateId: number,
  row: number,
  col: number,
): Promise<DensityMatrixCell | null> => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const cell = await readDensityMatrixCell(page, gateId, row, col)
    if (cell) {
      return cell
    }
    await page.waitForTimeout(50)
  }
  return null
}

const readBellDensityCells = async (page: TestPage): Promise<{
  diag: DensityMatrixCell | null
  offdiag: DensityMatrixCell | null
}> => ({
  diag: await readDensityCellUntilReady(page, 4, 0, 0),
  offdiag: await readDensityCellUntilReady(page, 4, 0, 1),
})

const summary = (cell: DensityMatrixCell | null) => cell && {
  gateId: cell.gateId,
  row: cell.row,
  col: cell.col,
  re: rounded(cell.re),
  im: rounded(cell.im),
  unity: rounded(cell.unity),
  span: cell.span,
}

const spanTwoDensityGateOrigin = () => ({
  x: EGUI_PANEL_MARGIN + UI_CONSTANTS.LINE_LEFT_OFFSET + UI_CONSTANTS.GATE_SIZE + UI_CONSTANTS.SLOT_SPACING - UI_CONSTANTS.GATE_SIZE / 2,
  y: EGUI_PANEL_MARGIN + UI_CONSTANTS.LINE_Y - UI_CONSTANTS.GATE_SIZE / 2,
})

const openSpanTwoDensityCircuit = async (page: TestPage): Promise<void> => {
  const payload = encodeURIComponent(JSON.stringify({ cols: [['H', 'H'], ['Density2']] }))
  await page.goto(`/#${payload}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await readDensityCellUntilReady(page, 3, 0, 0)
}

test('Density Matrix display captures a one-qubit reduced Bell state on the GPU', async ({ page }) => {
  const payload = encodeURIComponent(JSON.stringify({ cols: [['H'], ['•', 'X'], ['Density']] }))
  await page.goto(`/#${payload}`)

  await waitForStartupReady(page, { waitForStateVector: true })

  const { diag, offdiag } = await readBellDensityCells(page)

  expect({ diag: summary(diag), offdiag: summary(offdiag) }).toEqual({
    diag: { gateId: 4, row: 0, col: 0, re: 0.5, im: 0, unity: 1, span: 1 },
    offdiag: { gateId: 4, row: 0, col: 1, re: 0, im: 0, unity: 1, span: 1 },
  })
})

test('Density Matrix display captures coherent phase on the GPU', async ({ page }) => {
  const payload = encodeURIComponent(JSON.stringify({ cols: [['H'], ['S'], ['Density']] }))
  await page.goto(`/#${payload}`)

  await waitForStartupReady(page, { waitForStateVector: true })

  const cell = await readDensityCellUntilReady(page, 3, 0, 1)

  expect(summary(cell)).toEqual({ gateId: 3, row: 0, col: 1, re: 0, im: -0.5, unity: 1, span: 1 })
})

test('Density Matrix display renders span-two cells from the GPU buffer', async ({ page }) => {
  await openSpanTwoDensityCircuit(page)

  const canvas = page.locator('#egui-canvas')
  const gate = spanTwoDensityGateOrigin()

  await expect.poll(async () => {
    const pixels = await sampleCanvasPixels(page, canvas, [{ name: 'densityCell', x: gate.x + 12, y: gate.y + 12 }])
    return pixelRgbDistance(pixels.densityCell, DENSITY_CELL_FILL)
  }).toBeLessThan(80)
})

test('Density Matrix display omits internal cell separator lines', async ({ page }) => {
  await openSpanTwoDensityCircuit(page)

  const canvas = page.locator('#egui-canvas')
  const gate = spanTwoDensityGateOrigin()

  await expect.poll(async () => {
    const pixels = await sampleCanvasPixels(page, canvas, [{ name: 'separator', x: gate.x + 24, y: gate.y + 4 }])
    return pixelRgbDistance(pixels.separator, DENSITY_CELL_BACKGROUND)
  }).toBeLessThan(48)
})

test('Density Matrix display maximizes circle outlines without touching the outer frame', async ({ page }) => {
  await openSpanTwoDensityCircuit(page)

  const canvas = page.locator('#egui-canvas')
  const gate = spanTwoDensityGateOrigin()

  await expect.poll(async () => {
    const pixels = await sampleCanvasPixels(page, canvas, [
      { name: 'frameEdge', x: gate.x, y: gate.y + 12 },
      { name: 'nearCircle', x: gate.x + 1, y: gate.y + 12 },
    ])
    return {
      frameEdgeClear: pixelRgbDistance(pixels.frameEdge, DENSITY_MATRIX_BORDER) < 48,
      nearCircleVisible: pixelRgbDistance(pixels.nearCircle, DENSITY_CELL_BACKGROUND) > 48,
    }
  }).toEqual({ frameEdgeClear: true, nearCircleVisible: true })
})
