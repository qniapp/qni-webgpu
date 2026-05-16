import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  sampleCanvasPixels,
  waitForStartupReady,
  type CanvasPixel,
} from './support/egui-web-spec-helpers'

type Point = { x: number; y: number }
type ResizeGeometry = {
  items_height: number
  max_items_height: number
  items_top: number
  items_bottom: number
  handle_left: number
  handle_right: number
  handle_top: number
  handle_bottom: number
  footer_top: number
  footer_bottom: number
  scroll_offset_y: number
  hovered: boolean
  dragging: boolean
}

const TRIGGER: Point = { x: 40, y: 22 }
const EMPTY_JSON = '{"cols":[]}'
const FLEXOKI_BLUE_600: CanvasPixel = [32, 94, 166, 255]

const waitForCondition = async (
  page: Page,
  predicate: () => Promise<boolean>,
  description: string,
  attempts = 100,
): Promise<void> => {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (await predicate()) return
    await page.waitForTimeout(50)
  }
  throw new Error(`timed out waiting for ${description}`)
}

const canvasBox = async (page: Page) => {
  const box = await page.locator('#egui-canvas').boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  return box
}

const clickCanvas = async (page: Page, point: Point): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.click(box.x + point.x, box.y + point.y)
}

const seedCircuits = async (page: Page, count: number): Promise<void> => {
  await waitForCondition(page, async () => page.evaluate(() => typeof (window as any).__seedCircuits === 'function'), 'seed hook')
  const entries = Array.from({ length: count }, (_, index) => ({
    id: `circuit-${index + 1}`,
    name: `Circuit ${index + 1}`,
    circuit_json: EMPTY_JSON,
    updated_at: index + 1,
  }))
  await page.evaluate((entries) => {
    const seed = (window as any).__seedCircuits
    seed(JSON.stringify({ entries, active_id: 'circuit-1' }))
  }, entries)
}

const resizeGeometry = async (page: Page): Promise<ResizeGeometry | null> =>
  page.evaluate(() => {
    const raw = (window as any).__qniCircuitPickerResizeGeometryJson
    return typeof raw === 'string' ? JSON.parse(raw) as ResizeGeometry : null
  })

const tryResizeGeometry = async (
  page: Page,
  predicate: (geometry: ResizeGeometry) => boolean,
  attempts: number,
): Promise<ResizeGeometry | null> => {
  let last: ResizeGeometry | null = null
  try {
    await waitForCondition(page, async () => {
      last = await resizeGeometry(page)
      return last !== null && predicate(last)
    }, 'picker resize geometry', attempts)
  } catch {
    return null
  }
  return last
}

const waitForResizeGeometry = async (
  page: Page,
  predicate: (geometry: ResizeGeometry) => boolean = () => true,
  description = 'picker resize geometry',
): Promise<ResizeGeometry> => {
  const geometry = await tryResizeGeometry(page, predicate, 100)
  if (!geometry) throw new Error(`timed out waiting for ${description}`)
  return geometry
}

const openPicker = async (page: Page, count = 12): Promise<ResizeGeometry> => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
  await seedCircuits(page, count)
  await clickCanvas(page, TRIGGER)
  return waitForResizeGeometry(page)
}

const handleCenter = (geometry: ResizeGeometry): Point => ({
  x: (geometry.handle_left + geometry.handle_right) / 2,
  y: (geometry.handle_top + geometry.handle_bottom) / 2,
})

const footerCenter = (geometry: ResizeGeometry): Point => ({
  x: (geometry.handle_left + geometry.handle_right) / 2,
  y: (geometry.footer_top + geometry.footer_bottom) / 2,
})

const itemsCenter = (geometry: ResizeGeometry): Point => ({
  x: (geometry.handle_left + geometry.handle_right) / 2,
  y: (geometry.items_top + geometry.items_bottom) / 2,
})

const beginSeparatorDrag = async (page: Page, box: { x: number; y: number }, from: Point): Promise<void> => {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await page.mouse.move(box.x + from.x, box.y + from.y)
    await page.mouse.down()
    await page.mouse.move(box.x + from.x, box.y + from.y + 2, { steps: 2 })
    if (await tryResizeGeometry(page, (geometry) => geometry.dragging, 20)) return
    await page.mouse.up()
  }
  throw new Error('timed out waiting for resize drag start')
}

const dragSeparator = async (page: Page, deltaY: number): Promise<void> => {
  const box = await canvasBox(page)
  const from = handleCenter(await waitForResizeGeometry(page))
  await beginSeparatorDrag(page, box, from)
  await page.mouse.move(box.x + from.x, box.y + from.y + deltaY, { steps: 10 })
  await page.mouse.up()
}

const wheelCanvas = async (page: Page, point: Point, deltaY: number): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + point.x, box.y + point.y)
  await page.mouse.wheel(0, deltaY)
}

test('circuit picker separator drag grows the items pane by the pointer delta', async ({ page }) => {
  const before = await openPicker(page)
  await dragSeparator(page, 60)
  const after = await waitForResizeGeometry(page, (geometry) => Math.abs(geometry.items_height - before.items_height - 60) <= 1, 'items height grow')

  expect(Math.round(after.items_height - before.items_height)).toBe(60)
})

test('circuit picker separator drag clamps the items pane at the minimum height', async ({ page }) => {
  await openPicker(page)
  await dragSeparator(page, -400)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.items_height <= 73, 'items height minimum')

  expect(Math.round(after.items_height)).toBe(72)
})

test('circuit picker separator drag clamps the items pane at the maximum height', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 640 })
  await openPicker(page, 24)
  await dragSeparator(page, 800)
  const after = await waitForResizeGeometry(page, (geometry) => Math.abs(geometry.items_height - geometry.max_items_height) <= 1, 'items height maximum')

  expect(Math.round(after.items_height)).toBe(Math.round(after.max_items_height))
})

test('circuit picker maximum content height reveals all rows without residual item scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 1400 })
  await openPicker(page, 10)
  await dragSeparator(page, 800)
  const maxed = await waitForResizeGeometry(page, (geometry) => Math.abs(geometry.items_height - geometry.max_items_height) <= 1, 'content height maximum')
  await wheelCanvas(page, itemsCenter(maxed), 280)
  const box = await canvasBox(page)
  const handle = handleCenter(maxed)
  await page.mouse.move(box.x + handle.x, box.y + handle.y)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.hovered, 'resize handle hover after content-cap wheel')

  expect(Math.round(after.scroll_offset_y)).toBe(0)
})

test('circuit picker resize handle hover paints the separator in Flexoki blue-600', async ({ page }) => {
  const geometry = await openPicker(page)
  const box = await canvasBox(page)
  const point = handleCenter(geometry)
  await page.mouse.move(box.x + point.x, box.y + point.y)
  const hovered = await waitForResizeGeometry(page, (geometry) => geometry.hovered, 'resize handle hover')
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'handle-line', x: hovered.handle_left + 24, y: (hovered.handle_top + hovered.handle_bottom) / 2 },
  ])

  expect(pixelRgbDistance(pixels['handle-line'], FLEXOKI_BLUE_600) < 80).toBe(true)
})

test('circuit picker keeps the row-resize cursor while dragging outside the handle', async ({ page }) => {
  const geometry = await openPicker(page)
  const box = await canvasBox(page)
  const point = handleCenter(geometry)
  await beginSeparatorDrag(page, box, point)
  await page.mouse.move(box.x + point.x, box.y + point.y + 96, { steps: 8 })
  await page.waitForFunction(() => {
    const canvas = document.querySelector('#egui-canvas')
    return canvas && ['row-resize', 'ns-resize'].includes(getComputedStyle(canvas).cursor)
  })
  const cursor = await page.locator('#egui-canvas').evaluate((canvas) => getComputedStyle(canvas).cursor)
  await page.mouse.up()

  expect(['row-resize', 'ns-resize'].includes(cursor)).toBe(true)
})

test('circuit picker items pane wheel scroll changes only the pane scroll offset', async ({ page }) => {
  const before = await openPicker(page)
  await wheelCanvas(page, itemsCenter(before), 280)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.scroll_offset_y > before.scroll_offset_y + 8, 'items pane scroll')

  expect(after.scroll_offset_y > before.scroll_offset_y).toBe(true)
})

test('circuit picker footer stays pinned while the items pane scrolls', async ({ page }) => {
  const before = await openPicker(page)
  await wheelCanvas(page, itemsCenter(before), 280)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.scroll_offset_y > before.scroll_offset_y + 8, 'items pane scroll')

  expect(Math.abs(after.footer_top - before.footer_top) <= 0.5).toBe(true)
})

test('circuit picker footer wheel does not scroll the items pane', async ({ page }) => {
  const before = await openPicker(page)
  await wheelCanvas(page, footerCenter(before), 280)
  const box = await canvasBox(page)
  const handle = handleCenter(before)
  await page.mouse.move(box.x + handle.x, box.y + handle.y)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.hovered, 'resize handle hover after footer wheel')

  expect(Math.round(after.scroll_offset_y)).toBe(Math.round(before.scroll_offset_y))
})

test.describe('high-DPR picker resize', () => {
  test.use({ viewport: { width: 1000, height: 1400 }, deviceScaleFactor: 2 })

  test('circuit picker separator drag keeps CSS-pixel delta on high DPR screens', async ({ page }) => {
    const before = await openPicker(page)
    await dragSeparator(page, 60)
    const after = await waitForResizeGeometry(page, (geometry) => Math.abs(geometry.items_height - before.items_height - 60) <= 1, 'high-DPR items height grow')

    expect(Math.round(after.items_height - before.items_height)).toBe(60)
  })
})
