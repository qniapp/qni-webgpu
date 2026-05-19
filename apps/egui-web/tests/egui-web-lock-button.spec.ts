import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/egui-web-spec-helpers'

type CircuitLibrarySnapshot = {
  entries: Array<{
    id: string
    name: string
    circuit_json: string
    updated_at: number
    locked: boolean
    origin: { kind: string; locked?: boolean; origin_id?: string }
  }>
  active_id: string
  active_locked: boolean
  active_kind: string
}

type ButtonGeometry = { left: number; right: number; top: number; bottom: number; hovered: boolean; tooltip: string }

const BELL_JSON = '{"cols":[["H"],["•","X"]]}'
const H_JSON = '{"cols":[["H"]]}'
const X_JSON = '{"cols":[["X"]]}'

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
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const state = await snapshot(page)
    if (predicate(state)) return state
    await page.waitForTimeout(50)
  }
  throw new Error(`timed out waiting for ${description}`)
}

const waitForHashPayload = async (page: Page, expected: string): Promise<void> => {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const payload = await page.evaluate(() => decodeURIComponent(window.location.hash.slice(1)))
    if (payload === expected) return
    await page.waitForTimeout(50)
  }
  throw new Error(`timed out waiting for URL hash payload ${expected}`)
}

const canvasBox = async (page: Page) => {
  const box = await page.locator('#egui-canvas').boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  return box
}

const toolbarGeometry = async (page: Page, key: string): Promise<ButtonGeometry> => {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const raw = await page.evaluate((name) => (window as any)[name], key)
    if (typeof raw === 'string') return JSON.parse(raw) as ButtonGeometry
    await page.waitForTimeout(50)
  }
  throw new Error(`${key} was not published`)
}

const clickToolbarButton = async (page: Page, key: string): Promise<void> => {
  const box = await canvasBox(page)
  const geometry = await toolbarGeometry(page, key)
  await page.mouse.click(box.x + (geometry.left + geometry.right) / 2, box.y + (geometry.top + geometry.bottom) / 2)
}

const seedLibrary = async (page: Page, payload: unknown): Promise<void> => {
  await page.evaluate((library) => {
    const seed = (window as any).__seedCircuits
    if (typeof seed !== 'function') throw new Error('__seedCircuits hook missing')
    seed(JSON.stringify(library))
  }, payload)
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
})

test('sample circuits expose a disabled locked toolbar state', async ({ page }) => {
  await seedLibrary(page, {
    entries: [{ id: 'bell', name: 'Bell state', circuit_json: BELL_JSON, updated_at: 1, origin: { kind: 'sample', origin_id: 'bell' } }],
    active_id: 'bell',
  })
  await waitForSnapshot(page, (state) => state.active_kind === 'example', 'sample active')

  const geometry = await toolbarGeometry(page, '__qniToolbarLockGeometryJson')

  expect(geometry.tooltip).toBe('Locked (sample) — duplicate to edit')
})

test('duplicating a sample creates an unlocked My circuit', async ({ page }) => {
  await seedLibrary(page, {
    entries: [{ id: 'bell', name: 'Bell state', circuit_json: BELL_JSON, updated_at: 1, origin: { kind: 'sample', origin_id: 'bell' } }],
    active_id: 'bell',
  })
  await waitForSnapshot(page, (state) => state.active_kind === 'example', 'sample active')

  await clickToolbarButton(page, '__qniToolbarDuplicateGeometryJson')
  const state = await waitForSnapshot(page, (next) => next.active_kind === 'my', 'duplicate active')

  expect(state.entries.find((entry) => entry.id === state.active_id)?.locked).toBe(false)
})

test('locking a My circuit guards runtime URL apply', async ({ page }) => {
  await seedLibrary(page, {
    entries: [{ id: 'mine', name: 'Mine', circuit_json: H_JSON, updated_at: 1, origin: { kind: 'user', locked: false } }],
    active_id: 'mine',
  })
  await waitForSnapshot(page, (state) => state.active_locked === false, 'unlocked My active')

  await clickToolbarButton(page, '__qniToolbarLockGeometryJson')
  await waitForSnapshot(page, (state) => state.active_locked === true, 'My circuit locked')
  await page.evaluate((json) => {
    window.location.hash = encodeURIComponent(json)
  }, X_JSON)
  await page.evaluate((json) => {
    const apply = (window as any).__qniApplyUrlPayload
    if (typeof apply !== 'function') throw new Error('__qniApplyUrlPayload hook missing')
    apply(json)
  }, X_JSON)
  await waitForHashPayload(page, H_JSON)
  const state = await waitForSnapshot(page, (next) => next.active_locked === true, 'locked URL apply rejected')

  expect(state.entries.find((entry) => entry.id === 'mine')?.circuit_json).toBe(H_JSON)
})
