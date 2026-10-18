import { expect, test, type Page } from '@playwright/test'
import {
  dragPointer,
  getCircuitCellCenter,
  isDragPreviewFill,
  pixelRgbDistance,
  sampleCanvasPixels,
  waitForStartupReady,
  waitForValue,
  type CanvasPixel,
  type PixelSamplePoint,
  type Point,
} from './support/web-spec-helpers'

const VIEWPORT = { width: 1280, height: 800 }

test.use({ viewport: VIEWPORT })

const SOURCE: Point = getCircuitCellCenter(VIEWPORT.width)
const OFFGRID: Point = getCircuitCellCenter(VIEWPORT.width, 3, 4)
const SLOT: Point = getCircuitCellCenter(VIEWPORT.width, 2)
const LABEL: CanvasPixel = [255, 252, 240, 255]

const BODY_OFFSETS: Record<string, Point> = {
  top: { x: 0, y: -17 },
  bottom: { x: 0, y: 17 },
  topLeft: { x: -17, y: -12 },
  bottomRight: { x: 17, y: 12 },
}

const bodyPoints = (center: Point): PixelSamplePoint[] =>
  Object.entries(BODY_OFFSETS).map(([name, offset]) => ({
    name,
    x: center.x + offset.x,
    y: center.y + offset.y,
  }))

const gridPoints = (center: Point): PixelSamplePoint[] => {
  const points: PixelSamplePoint[] = []
  for (let dy = -18; dy <= 18; dy += 2) {
    for (let dx = -18; dx <= 18; dx += 2) {
      points.push({ name: `${dx},${dy}`, x: center.x + dx, y: center.y + dy })
    }
  }
  return points
}

const readBodyFill = async (page: Page, center: Point): Promise<Record<string, boolean>> => {
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), bodyPoints(center))
  return Object.fromEntries(Object.entries(pixels).map(([name, pixel]) => [name, isDragPreviewFill(pixel)]))
}

const allBodyPoints = (value: boolean): Record<string, boolean> =>
  Object.fromEntries(Object.keys(BODY_OFFSETS).map(name => [name, value]))

const openCircuit = async (page: Page, op: string): Promise<void> => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [[op]] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
}

const cancelHeldDrag = async (page: Page): Promise<void> => {
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await page.keyboard.up('Shift')
}

test('held |0> drag paints the grabbed purple body', async ({ page }) => {
  await openCircuit(page, '|0>')
  await dragPointer(page, SOURCE, OFFGRID, 8, false)
  await expect.poll(() => readBodyFill(page, OFFGRID)).toEqual(allBodyPoints(true))
  await cancelHeldDrag(page)
})

test('held Shift+drag |0> copy paints the grabbed purple body', async ({ page }) => {
  await openCircuit(page, '|0>')
  await page.keyboard.down('Shift')
  await dragPointer(page, SOURCE, OFFGRID, 8, false)
  await expect.poll(() => readBodyFill(page, OFFGRID)).toEqual(allBodyPoints(true))
  await cancelHeldDrag(page)
})

test('held Shift+drag |0> keeps the original in its normal look', async ({ page }) => {
  await openCircuit(page, '|0>')
  const points = [...bodyPoints(SOURCE), { name: 'digit', ...SOURCE }]
  const before = await sampleCanvasPixels(page, page.locator('#egui-canvas'), points)
  await page.keyboard.down('Shift')
  await dragPointer(page, SOURCE, OFFGRID, 8, false)
  await waitForValue(
    () => readBodyFill(page, OFFGRID),
    fill => Object.values(fill).every(Boolean),
    'expected the held copy before sampling the original'
  )
  const during = await sampleCanvasPixels(page, page.locator('#egui-canvas'), points)
  const unchanged = Object.fromEntries(
    Object.keys(before).map(name => [name, pixelRgbDistance(before[name], during[name]) <= 24])
  )
  await cancelHeldDrag(page)
  expect(unchanged).toEqual(Object.fromEntries(points.map(({ name }) => [name, true])))
})

test('snapped Measure drag paints the purple body with a light outcome digit', async ({ page }) => {
  await openCircuit(page, 'Measure')
  await dragPointer(page, SOURCE, SLOT, 8, false)
  const readCounts = async () => {
    const pixels = Object.values(await sampleCanvasPixels(page, page.locator('#egui-canvas'), gridPoints(SLOT)))
    return {
      purpleBody: pixels.filter(isDragPreviewFill).length > 200,
      lightDigit: pixels.filter(pixel => pixelRgbDistance(pixel, LABEL) <= 24).length > 8,
    }
  }
  await expect.poll(readCounts).toEqual({ purpleBody: true, lightDigit: true })
  await cancelHeldDrag(page)
})

test('snapped Bloch drag outlines the sphere in purple without a square body', async ({ page }) => {
  await openCircuit(page, 'Bloch')
  await dragPointer(page, SOURCE, SLOT, 8, false)
  const readOutline = async () => {
    const diagonal = [12, 13, 14, 15, 16].map(d => ({ name: `outline${d}`, x: SLOT.x - d, y: SLOT.y + d }))
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
      ...diagonal,
      { name: 'corner', x: SLOT.x - 18, y: SLOT.y + 18 },
    ])
    return {
      outline: diagonal.some(({ name }) => isDragPreviewFill(pixels[name])),
      corner: isDragPreviewFill(pixels.corner),
    }
  }
  await expect.poll(readOutline).toEqual({ outline: true, corner: false })
  await cancelHeldDrag(page)
})
