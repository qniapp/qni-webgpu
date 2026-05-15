import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/egui-web-spec-helpers'

type CircuitLibrarySnapshot = {
  entries: Array<{ id: string; name: string; circuit_json: string; updated_at: number }>
  active_id: string
}

type Point = { x: number; y: number }

const ONE_JSON = '{"cols":[["H"]]}'
const TWO_JSON = '{"cols":[["X"]]}'
const THREE_JSON = '{"cols":[["QFT4"]]}'
const STORAGE_KEY = 'qni.circuit_library.v1'

const TRIGGER: Point = { x: 40, y: 22 }
const ROW_1: Point = { x: 80, y: 74 }
const ROW_2: Point = { x: 80, y: 110 }
const ROW_3: Point = { x: 80, y: 146 }
const KEBAB_X = 226
const SUBMENU_X = 320
const MOVE_DOWN_Y = 196

const snapshot = async (page: Page): Promise<CircuitLibrarySnapshot> => {
  const raw = await page.evaluate(() => {
    const getter = (window as any).__qniCircuitPickerSnapshot
    if (typeof getter !== 'function') throw new Error('__qniCircuitPickerSnapshot hook missing')
    return getter()
  })
  return JSON.parse(raw) as CircuitLibrarySnapshot
}

const seedLibrary = async (page: Page, activeId = 'two'): Promise<void> => {
  const library = {
    entries: [
      { id: 'one', name: 'One', circuit_json: ONE_JSON, updated_at: 1 },
      { id: 'two', name: 'Two', circuit_json: TWO_JSON, updated_at: 2 },
      { id: 'three', name: 'Three', circuit_json: THREE_JSON, updated_at: 3 },
    ],
    active_id: activeId,
  }
  await page.evaluate((payload) => {
    const seed = (window as any).__seedCircuits
    if (typeof seed !== 'function') throw new Error('__seedCircuits hook missing')
    seed(JSON.stringify(payload))
  }, library)
  await expect.poll(async () => (await snapshot(page)).active_id).toBe(activeId)
}

const canvasBox = async (page: Page) => {
  const box = await page.locator('#egui-canvas').boundingBox()
  expect(box).not.toBeNull()
  return box!
}

const clickCanvas = async (page: Page, point: Point): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.click(box.x + point.x, box.y + point.y)
}

const dragCanvas = async (page: Page, from: Point, to: Point): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + from.x, box.y + from.y)
  await page.mouse.down()
  await page.mouse.move(box.x + to.x, box.y + to.y, { steps: 8 })
  await page.mouse.up()
}

const dragCanvasAndCancel = async (page: Page, from: Point, to: Point): Promise<void> => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + from.x, box.y + from.y)
  await page.mouse.down()
  await page.mouse.move(box.x + to.x, box.y + to.y, { steps: 8 })
  await page.keyboard.press('Escape')
  await page.mouse.move(box.x + to.x, box.y + to.y - 24, { steps: 4 })
  await page.mouse.up()
}

const entryIds = async (page: Page): Promise<string[]> => (await snapshot(page)).entries.map((entry) => entry.id)

const storedEntryIds = async (page: Page): Promise<string[]> =>
  page.evaluate((key) => {
    const stored = JSON.parse(localStorage.getItem(key) ?? 'null')
    return stored?.circuits?.map((entry: { id: string }) => entry.id) ?? []
  }, STORAGE_KEY)

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
  await seedLibrary(page)
  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(200)
})

test('dragging the third item above the first reorders to [3, 1, 2]', async ({ page }) => {
  await dragCanvas(page, ROW_3, { x: ROW_1.x + 48, y: ROW_1.y - 18 })

  await expect.poll(async () => entryIds(page)).toEqual(['three', 'one', 'two'])
  expect((await snapshot(page)).active_id).toBe('two')
  await expect.poll(async () => storedEntryIds(page)).toEqual(['three', 'one', 'two'])

  await page.reload()
  await waitForStartupReady(page, { waitForStateVector: true })
  await expect.poll(async () => entryIds(page)).toEqual(['three', 'one', 'two'])
  expect((await snapshot(page)).active_id).toBe('two')
})

test('dragging the first item to the end reorders to [2, 3, 1]', async ({ page }) => {
  await dragCanvas(page, ROW_1, { x: ROW_3.x + 36, y: ROW_3.y + 28 })

  await expect.poll(async () => entryIds(page)).toEqual(['two', 'three', 'one'])
  expect((await snapshot(page)).active_id).toBe('two')
})

test('Escape while dragging cancels reorder', async ({ page }) => {
  await dragCanvasAndCancel(page, ROW_3, { x: ROW_1.x + 48, y: ROW_1.y - 18 })

  await expect.poll(async () => entryIds(page)).toEqual(['one', 'two', 'three'])
  expect((await snapshot(page)).active_id).toBe('two')
})

test('kebab click opens the submenu without starting a drag', async ({ page }) => {
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })
  await page.waitForTimeout(200)
  await clickCanvas(page, { x: SUBMENU_X, y: MOVE_DOWN_Y })

  await expect.poll(async () => entryIds(page)).toEqual(['two', 'one', 'three'])
})
