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

const BELL_JSON = '{"cols":[["H"]]}'
const GHZ_JSON = '{"cols":[["X"]]}'
const QFT_JSON = '{"cols":[["QFT4"]]}'
const STORAGE_KEY = 'qni.circuit_library.v1'

const TRIGGER: Point = { x: 80, y: 22 }
const ROW_1: Point = { x: 80, y: 74 }
const ROW_2: Point = { x: 80, y: 110 }
const FOOTER: Point = { x: 90, y: 223 }
const ROW_3: Point = { x: 80, y: 146 }
const ROW_4: Point = { x: 80, y: 182 }
const KEBAB_X = 226
const SUBMENU_X = 320
const MOVE_UP_SUBMENU_Y = 232
const FLEXOKI_BG: CanvasPixel = [255, 252, 240, 255] // Flexoki bg #FFFCF0

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

const waitForSnapshot = async (
  page: Page,
  predicate: (state: CircuitLibrarySnapshot) => boolean,
  description: string,
): Promise<CircuitLibrarySnapshot> => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    try {
      const state = await snapshot(page)
      if (predicate(state)) return state
    } catch (error) {
      if (!(error instanceof Error) || !error.message.includes('__qniCircuitPickerSnapshot hook missing')) {
        throw error
      }
    }
    await page.waitForTimeout(50)
  }
  throw new Error(`timed out waiting for circuit picker snapshot: ${description}`)
}

const storedDocument = async (page: Page): Promise<any> =>
  page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? 'null'), STORAGE_KEY)

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
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  await page.mouse.click((box?.x ?? 0) + point.x, (box?.y ?? 0) + point.y)
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
})

test('startup current entry preserves the three seeded sample circuits', async ({ page }) => {
  const state = await snapshot(page)

  expect({
    activeId: state.active_id,
    current: state.entries.find((entry) => entry.id === 'current'),
    bell: state.entries.find((entry) => entry.id === 'bell'),
    ghzName: state.entries.find((entry) => entry.id === 'ghz')?.name,
    qftName: state.entries.find((entry) => entry.id === 'qft-4')?.name,
    hashCols: readCircuitColsFromHash(page.url()),
  }).toMatchObject({
    activeId: 'current',
    current: { name: 'Circuit 1', circuit_json: '{"cols":[]}' },
    bell: { name: 'Bell state', circuit_json: '{"cols":[["H"],["•","X"]]}' },
    ghzName: 'GHZ state',
    qftName: 'QFT 4-qubit',
    hashCols: [],
  })
})

test('localStorage active circuit hydrates the picker and URL on reload', async ({ page }) => {
  await page.evaluate((key) => {
    localStorage.setItem(key, JSON.stringify({
      version: 1,
      activeId: 'stored-x',
      circuits: [{
        id: 'stored-x',
        name: 'Stored X',
        json: '{"cols":[["X"]]}',
        createdAt: 1,
        updatedAt: 1,
        meta: { qubits: 1, columns: 1, gateCount: 1 },
      }],
    }))
  }, STORAGE_KEY)
  await page.goto('/')
  await waitForStartupReady(page)

  const state = await waitForSnapshot(page, (next) => next.active_id === 'stored-x', 'stored circuit active')
  expect({ entry: state.entries[0], hashCols: readCircuitColsFromHash(page.url()) }).toMatchObject({
    entry: { name: 'Stored X', circuit_json: '{"cols":[["X"]]}' },
    hashCols: [['X']],
  })
})

test('startup does not overwrite unsupported localStorage documents', async ({ page }) => {
  await page.evaluate((key) => {
    localStorage.setItem(key, JSON.stringify({ version: 2, activeId: null, circuits: [] }))
  }, STORAGE_KEY)
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  expect(await storedDocument(page)).toEqual({ version: 2, activeId: null, circuits: [] })
})

test('URL payload wins over a different persisted active circuit', async ({ page }) => {
  await page.evaluate((key) => {
    localStorage.setItem(key, JSON.stringify({
      version: 1,
      activeId: 'stored-x',
      circuits: [{
        id: 'stored-x',
        name: 'Stored X',
        json: '{"cols":[["X"]]}',
        createdAt: 1,
        updatedAt: 1,
        meta: { qubits: 1, columns: 1, gateCount: 1 },
      }],
    }))
  }, STORAGE_KEY)
  await page.goto('/#' + encodeURIComponent('{"cols":[["H"]]}'))
  await waitForStartupReady(page, { waitForStateVector: true })

  const state = await waitForSnapshot(page, (next) => next.active_id === 'current', 'URL payload current entry')
  expect({ activeId: state.active_id, hashCols: readCircuitColsFromHash(page.url()) }).toEqual({
    activeId: 'current',
    hashCols: [['H']],
  })
})

test('low-level localStorage save hook refreshes the live picker', async ({ page }) => {
  const id = await page.evaluate(() => {
    const save = (window as any).__qniCircuitLibrarySave
    if (typeof save !== 'function') throw new Error('__qniCircuitLibrarySave hook missing')
    return save('Saved X', '{"cols":[["X"]]}')
  })

  const state = await waitForSnapshot(page, (next) => next.active_id === id, 'saved circuit active')
  expect({ entry: state.entries[0], hashCols: readCircuitColsFromHash(page.url()) }).toMatchObject({
    entry: { id, name: 'Saved X', circuit_json: '{"cols":[["X"]]}' },
    hashCols: [['X']],
  })
})

test('circuit picker opens and selecting another item syncs the URL hash', async ({ page }) => {
  await seedLibrary(page)

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, ROW_2)

  const selected = await waitForSnapshot(page, (state) => state.active_id === 'ghz', 'selected GHZ entry')
  const hashAfterSelect = readCircuitColsFromHash(page.url())

  await page.reload()
  await waitForStartupReady(page)
  const reloaded = await waitForSnapshot(page, (state) => state.active_id === 'ghz', 'reloaded GHZ entry')
  expect({ selected: selected.active_id, hashAfterSelect, reloaded: reloaded.active_id }).toEqual({
    selected: 'ghz',
    hashAfterSelect: [['X']],
    reloaded: 'ghz',
  })
})

test('Create new circuit adds an empty numbered Circuit entry and makes it active', async ({ page }) => {
  await seedLibrary(page)

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, FOOTER)

  const state = await waitForSnapshot(page, (next) => next.entries.length === 4, 'new circuit appended')
  const lastEntry = state.entries.at(-1)
  const stored = await storedDocument(page)
  expect({
    lastEntry,
    activeId: state.active_id,
    hashCols: readCircuitColsFromHash(page.url()),
    storedActiveId: stored.activeId,
    storedLast: stored.circuits.at(-1),
  }).toMatchObject({
    lastEntry: { name: 'Circuit 1', circuit_json: '{"cols":[]}' },
    activeId: lastEntry?.id,
    hashCols: [],
    storedActiveId: lastEntry?.id,
    storedLast: { name: 'Circuit 1', json: '{"cols":[]}' },
  })
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

test('Rename action accepts Japanese circuit names', async ({ page }) => {
  await seedLibrary(page)

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_1.y })
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: SUBMENU_X, y: 88 })
  await page.waitForTimeout(300)
  await page.keyboard.type('量子回路')
  await page.keyboard.press('Enter')

  await expect.poll(async () => (await snapshot(page)).entries[0].name).toBe('量子回路')
})

test('Move up keeps the displaced bottom item visually idle', async ({ page }) => {
  const library = {
    entries: [
      { id: 'current', name: 'Circuit 1', circuit_json: '{"cols":[]}', updated_at: 0 },
      { id: 'bell', name: 'Bell state', circuit_json: BELL_JSON, updated_at: 1 },
      { id: 'ghz', name: 'GHZ state', circuit_json: GHZ_JSON, updated_at: 2 },
      { id: 'qft', name: 'QFT 4-qubit', circuit_json: QFT_JSON, updated_at: 3 },
    ],
    active_id: 'qft',
  }
  await page.evaluate((payload) => {
    const seed = (window as any).__seedCircuits
    if (typeof seed !== 'function') throw new Error('__seedCircuits hook missing')
    seed(JSON.stringify(payload))
  }, library)
  await waitForSnapshot(page, (state) => state.active_id === 'qft', 'seeded QFT active')

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_4.y })
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: SUBMENU_X, y: MOVE_UP_SUBMENU_Y + 36 })
  await page.waitForTimeout(300)

  const state = await waitForSnapshot(
    page,
    (next) => next.entries.map((entry) => entry.id).join(',') === 'current,bell,qft,ghz',
    'QFT moved above GHZ',
  )
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'bottomRowBg', x: 180, y: ROW_4.y },
  ])
  expect({
    order: state.entries.map((entry) => entry.id),
    bottomRowIdle: pixelRgbDistance(pixels.bottomRowBg, FLEXOKI_BG) < 10,
  }).toEqual({
    order: ['current', 'bell', 'qft', 'ghz'],
    bottomRowIdle: true,
  })
})

test('Delete active circuit falls back to the first remaining entry', async ({ page }) => {
  await seedLibrary(page, 'ghz')

  await clickCanvas(page, TRIGGER)
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: KEBAB_X, y: ROW_2.y })
  await page.waitForTimeout(300)
  await clickCanvas(page, { x: SUBMENU_X, y: 302 })

  const state = await waitForSnapshot(page, (next) => next.active_id === 'bell', 'delete fallback active')
  expect({
    activeId: state.active_id,
    entries: state.entries.map((entry) => entry.id),
    hashCols: readCircuitColsFromHash(page.url()),
  }).toEqual({
    activeId: 'bell',
    entries: ['bell', 'qft'],
    hashCols: [['H']],
  })
})
