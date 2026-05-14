import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/egui-web-spec-helpers'

type CircuitLibrarySnapshot = {
  entries: Array<{ id: string; name: string; circuit_json: string; updated_at: number }>
  active_id: string
}

type Point = { x: number; y: number }

const BELL_JSON = '{"cols":[["H"]]}'
const GHZ_JSON = '{"cols":[["X"]]}'
const QFT_JSON = '{"cols":[["QFT4"]]}'

const TRIGGER: Point = { x: 80, y: 22 }
const ROW_1: Point = { x: 80, y: 74 }
const ROW_2: Point = { x: 80, y: 110 }
const FOOTER: Point = { x: 90, y: 195 }
const KEBAB_X = 226
const SUBMENU_X = 320

const readCircuitColsFromHash = (url: string): unknown[] => {
  const hash = new URL(url).hash.slice(1)
  if (!hash) return []
  return JSON.parse(decodeURIComponent(hash)).cols
}

const snapshot = async (page: Page): Promise<CircuitLibrarySnapshot> => {
  const raw = await page.evaluate(() => {
    const getter = (window as any).__qniCircuitPickerSnapshot
    if (typeof getter !== 'function') throw new Error('__qniCircuitPickerSnapshot hook missing')
    return getter()
  })
  return JSON.parse(raw) as CircuitLibrarySnapshot
}

const seedLibrary = async (page: Page, activeId = 'bell'): Promise<void> => {
  const library = {
    entries: [
      { id: 'bell', name: 'Bell state', circuit_json: BELL_JSON, updated_at: 1 },
      { id: 'ghz', name: 'GHZ state', circuit_json: GHZ_JSON, updated_at: 2 },
      { id: 'qft', name: 'QFT 4-qubit', circuit_json: QFT_JSON, updated_at: 3 },
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

const clickCanvas = async (page: Page, point: Point): Promise<void> => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  await page.mouse.click((box?.x ?? 0) + point.x, (box?.y ?? 0) + point.y)
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
})

test('startup current entry preserves the three seeded sample circuits', async ({ page }) => {
  const state = await snapshot(page)

  expect(state.active_id).toBe('current')
  expect(state.entries.find((entry) => entry.id === 'bell')).toMatchObject({
    name: 'Bell state',
    circuit_json: '{"cols":[["H"],["•","X"]]}',
  })
  expect(state.entries.find((entry) => entry.id === 'ghz')).toMatchObject({ name: 'GHZ state' })
  expect(state.entries.find((entry) => entry.id === 'qft-4')).toMatchObject({ name: 'QFT 4-qubit' })
  expect(readCircuitColsFromHash(page.url())).toEqual([])
})

test('circuit picker opens and selecting another item syncs the URL hash', async ({ page }) => {
  await seedLibrary(page)

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, ROW_2)

  await expect.poll(async () => (await snapshot(page)).active_id).toBe('ghz')
  expect(readCircuitColsFromHash(page.url())).toEqual([['X']])
})

test('Create new circuit adds an empty Untitled entry and makes it active', async ({ page }) => {
  await seedLibrary(page)

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, FOOTER)

  await expect.poll(async () => (await snapshot(page)).entries.length).toBe(4)
  const state = await snapshot(page)
  expect(state.entries.at(-1)).toMatchObject({ name: 'Untitled', circuit_json: '{"cols":[]}' })
  expect(state.active_id).toBe(state.entries.at(-1)?.id)
  expect(readCircuitColsFromHash(page.url())).toEqual([])
})

test('Rename action turns the item into an inline editor and commits on Enter', async ({ page }) => {
  await seedLibrary(page)

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: SUBMENU_X, y: 88 })
  await page.waitForTimeout(300)
  await page.keyboard.type('Renamed Bell')
  await page.keyboard.press('Enter')

  await expect.poll(async () => (await snapshot(page)).entries[0].name).toBe('Renamed Bell')
})

test('Delete active circuit falls back to the first remaining entry', async ({ page }) => {
  await seedLibrary(page, 'ghz')

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_2.y })
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: SUBMENU_X, y: 302 })

  await expect.poll(async () => (await snapshot(page)).active_id).toBe('bell')
  const state = await snapshot(page)
  expect(state.entries.map((entry) => entry.id)).toEqual(['bell', 'qft'])
  expect(readCircuitColsFromHash(page.url())).toEqual([['H']])
})
