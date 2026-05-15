import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  sampleCanvasPixels,
  waitForStartupReady,
  type CanvasPixel,
} from './support/egui-web-spec-helpers'

type CircuitLibrarySnapshot = {
  entries: Array<{ id: string; name: string; circuit_json: string; updated_at: number }>
  active_id: string
}

type Point = { x: number; y: number }
type HoverSnapshot = { hoveredGateId: number | null; hoveredPaletteIndex: number | null }

const ONE_JSON = '{"cols":[["H"]]}'
const TWO_JSON = '{"cols":[["X"]]}'
const THREE_JSON = '{"cols":[["QFT4"]]}'
const STORAGE_KEY = 'qni.circuit_library.v1'
const FLEXOKI_BG: CanvasPixel = [255, 252, 240, 255]
const FLEXOKI_BG_2: CanvasPixel = [242, 240, 229, 255]
const FLEXOKI_UI: CanvasPixel = [230, 228, 217, 255]
const FLEXOKI_TX: CanvasPixel = [16, 15, 15, 255]

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

const hoverSnapshot = async (page: Page): Promise<HoverSnapshot> =>
  page.evaluate(() => JSON.parse((window as any).__qniHoverSnapshotJson ?? '{}') as HoverSnapshot)

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

test('pressed row shows dragged styling before movement threshold', async ({ page }) => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + ROW_2.x, box.y + ROW_2.y)
  await page.mouse.down()
  try {
    await page.waitForTimeout(80)
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
      { name: 'pressed-row-fill', x: 180, y: ROW_2.y },
      { name: 'next-row-paper', x: 180, y: ROW_2.y + 24 },
    ])
    const fill = pixels['pressed-row-fill']
    expect(pixelRgbDistance(fill, FLEXOKI_UI)).toBeLessThan(40)
    expect(pixelRgbDistance(fill, FLEXOKI_BG)).toBeGreaterThan(45)
    expect(pixelRgbDistance(pixels['next-row-paper'], FLEXOKI_BG)).toBeLessThan(25)
    await expect.poll(async () => entryIds(page)).toEqual(['one', 'two', 'three'])
  } finally {
    await page.mouse.up()
  }
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

test('picker overlay suppresses underlying circuit and palette hover', async ({ page }) => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + KEBAB_X, box.y + ROW_2.y)

  await expect.poll(async () => await hoverSnapshot(page)).toMatchObject({
    hoveredGateId: null,
    hoveredPaletteIndex: null,
  })
})

test('kebab hover darkens only the dots without painting a chip', async ({ page }) => {
  const box = await canvasBox(page)
  await page.mouse.move(box.x + KEBAB_X, box.y + ROW_1.y)
  await page.waitForTimeout(120)

  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'row-hover-fill-under-kebab', x: KEBAB_X - 7, y: ROW_1.y },
    { name: 'hover-dot', x: KEBAB_X, y: ROW_1.y },
  ])

  expect(pixelRgbDistance(pixels['row-hover-fill-under-kebab'], FLEXOKI_BG_2)).toBeLessThan(25)
  expect(pixelRgbDistance(pixels['row-hover-fill-under-kebab'], FLEXOKI_UI)).toBeGreaterThan(20)
  expect(pixelRgbDistance(pixels['hover-dot'], FLEXOKI_TX)).toBeLessThan(80)
})

test('starting a row drag closes an open kebab submenu', async ({ page }) => {
  const box = await canvasBox(page)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })
  await page.waitForTimeout(160)

  await page.mouse.move(box.x + ROW_3.x, box.y + ROW_3.y)
  await page.mouse.down()
  await page.mouse.move(box.x + ROW_3.x, box.y + ROW_3.y + 8, { steps: 4 })
  await page.mouse.up()

  await clickCanvas(page, { x: SUBMENU_X, y: MOVE_DOWN_Y })
  await expect.poll(async () => entryIds(page)).toEqual(['one', 'two', 'three'])
})

test('clicking the open kebab trigger closes its submenu', async ({ page }) => {
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })
  await page.waitForTimeout(160)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })

  await clickCanvas(page, { x: SUBMENU_X, y: MOVE_DOWN_Y })
  await expect.poll(async () => entryIds(page)).toEqual(['one', 'two', 'three'])
})

test('kebab click opens the submenu without starting a drag', async ({ page }) => {
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })
  await page.waitForTimeout(200)
  await clickCanvas(page, { x: SUBMENU_X, y: MOVE_DOWN_Y })

  await expect.poll(async () => entryIds(page)).toEqual(['two', 'one', 'three'])
})
