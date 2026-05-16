import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/egui-web-spec-helpers'

type Point = { x: number; y: number }
type ResizeGeometry = {
  items_top: number
  items_bottom: number
  handle_left: number
  handle_right: number
  first_row_top: number
  scroll_offset_y: number
}
type CircuitLibrarySnapshot = {
  entries: Array<{ id: string; name: string; circuit_json: string; updated_at: number }>
  active_id: string
}

const TRIGGER: Point = { x: 40, y: 22 }
const EMPTY_JSON = '{"cols":[]}'

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
  await page.evaluate((payload) => {
    const seed = (window as any).__seedCircuits
    seed(JSON.stringify({ entries: payload, active_id: 'circuit-1' }))
  }, entries)
}

const resizeGeometry = async (page: Page): Promise<ResizeGeometry | null> =>
  page.evaluate(() => {
    const raw = (window as any).__qniCircuitPickerResizeGeometryJson
    return typeof raw === 'string' ? JSON.parse(raw) as ResizeGeometry : null
  })

const waitForResizeGeometry = async (
  page: Page,
  predicate: (geometry: ResizeGeometry) => boolean = () => true,
  description = 'picker resize geometry',
): Promise<ResizeGeometry> => {
  let last: ResizeGeometry | null = null
  await waitForCondition(page, async () => {
    last = await resizeGeometry(page)
    return last !== null && predicate(last)
  }, description)
  if (!last) throw new Error('picker resize geometry missing')
  return last
}

const snapshot = async (page: Page): Promise<CircuitLibrarySnapshot> => {
  const raw = await page.evaluate(() => {
    const getter = (window as any).__qniCircuitPickerSnapshot
    if (typeof getter !== 'function') throw new Error('__qniCircuitPickerSnapshot hook missing')
    return getter()
  })
  return JSON.parse(raw) as CircuitLibrarySnapshot
}

const entryIds = async (page: Page): Promise<string[]> => (await snapshot(page)).entries.map((entry) => entry.id)

const openPicker = async (page: Page, count = 16): Promise<ResizeGeometry> => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
  await seedCircuits(page, count)
  await clickCanvas(page, TRIGGER)
  return waitForResizeGeometry(page)
}

const wheelCanvas = async (page: Page, point: Point, deltaY: number): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + point.x, box.y + point.y)
  await page.mouse.wheel(0, deltaY)
}

const itemsCenter = (geometry: ResizeGeometry): Point => ({
  x: (geometry.handle_left + geometry.handle_right) / 2,
  y: (geometry.items_top + geometry.items_bottom) / 2,
})

const visibleDragStart = (geometry: ResizeGeometry): Point => ({
  x: geometry.handle_left + 64,
  y: geometry.items_top + 72,
})

const firstRowDragStart = (geometry: ResizeGeometry): Point => ({
  x: geometry.handle_left + 64,
  y: geometry.first_row_top + 18,
})

const beginItemDrag = async (page: Page, point: Point): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + point.x, box.y + point.y)
  await page.mouse.down()
  await page.mouse.move(box.x + point.x, box.y + point.y + 8, { steps: 3 })
}

const moveHeldPointer = async (page: Page, point: Point): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + point.x, box.y + point.y, { steps: 8 })
}

test('drag-to-reorder top edge auto-scroll decreases the items pane offset', async ({ page }) => {
  const opened = await openPicker(page, 18)
  await wheelCanvas(page, itemsCenter(opened), 360)
  const before = await waitForResizeGeometry(page, (geometry) => geometry.scroll_offset_y > 80, 'pre-scrolled items pane')
  await beginItemDrag(page, visibleDragStart(before))
  await moveHeldPointer(page, { x: before.handle_left + 64, y: before.items_top + 8 })
  await page.waitForTimeout(700)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.scroll_offset_y < before.scroll_offset_y - 20, 'top edge auto-scroll')
  await page.mouse.up()

  expect(after.scroll_offset_y < before.scroll_offset_y).toBe(true)
})

test('drag-to-reorder bottom edge auto-scroll increases the items pane offset', async ({ page }) => {
  const before = await openPicker(page, 18)
  await beginItemDrag(page, firstRowDragStart(before))
  await moveHeldPointer(page, { x: before.handle_left + 64, y: before.items_bottom - 8 })
  await page.waitForTimeout(700)
  const after = await waitForResizeGeometry(page, (geometry) => geometry.scroll_offset_y > before.scroll_offset_y + 20, 'bottom edge auto-scroll')
  await page.mouse.up()

  expect(after.scroll_offset_y > before.scroll_offset_y).toBe(true)
})

test('drag-to-reorder commits the auto-scrolled slot on mouseup', async ({ page }) => {
  const before = await openPicker(page, 12)
  await beginItemDrag(page, firstRowDragStart(before))
  await moveHeldPointer(page, { x: before.handle_left + 64, y: before.items_bottom - 8 })
  await waitForResizeGeometry(page, (geometry) => geometry.scroll_offset_y > 220, 'bottom auto-scroll reaches the final slot')
  await page.mouse.up()
  await waitForCondition(page, async () => (await entryIds(page)).at(-1) === 'circuit-1', 'auto-scrolled reorder commit')

  expect((await entryIds(page)).join(',')).toBe('circuit-2,circuit-3,circuit-4,circuit-5,circuit-6,circuit-7,circuit-8,circuit-9,circuit-10,circuit-11,circuit-12,circuit-1')
})
