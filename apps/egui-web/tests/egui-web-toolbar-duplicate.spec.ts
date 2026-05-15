import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/egui-web-spec-helpers'

type CircuitLibrarySnapshot = {
  entries: Array<{ id: string; name: string; circuit_json: string; updated_at: number }>
  active_id: string
}

type ToolbarDuplicateGeometry = {
  left: number
  right: number
  top: number
  bottom: number
  hovered: boolean
}

const STORAGE_KEY = 'qni.circuit_library.v1'

const snapshot = async (page: Page): Promise<CircuitLibrarySnapshot> => {
  const raw = await page.evaluate(() => {
    const getter = (window as any).__qniCircuitPickerSnapshot
    if (typeof getter !== 'function') throw new Error('__qniCircuitPickerSnapshot hook missing')
    return getter()
  })
  return JSON.parse(raw) as CircuitLibrarySnapshot
}

const duplicateGeometry = async (page: Page): Promise<ToolbarDuplicateGeometry> => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const raw = await page.evaluate(() => (window as any).__qniToolbarDuplicateGeometryJson)
    if (typeof raw === 'string') return JSON.parse(raw) as ToolbarDuplicateGeometry
    await page.waitForTimeout(50)
  }
  throw new Error('toolbar Duplicate geometry was not published')
}

const waitForSnapshot = async (
  page: Page,
  predicate: (state: CircuitLibrarySnapshot) => boolean,
  description: string,
): Promise<CircuitLibrarySnapshot> => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const state = await snapshot(page)
    if (predicate(state)) return state
    await page.waitForTimeout(50)
  }
  throw new Error(`timed out waiting for circuit picker snapshot: ${description}`)
}

const canvasBox = async (page: Page) => {
  const box = await page.locator('#egui-canvas').boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }
  return box!
}

const duplicateCenter = async (page: Page) => {
  const rect = await duplicateGeometry(page)
  return { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 }
}

const clickDuplicate = async (page: Page): Promise<void> => {
  const box = await canvasBox(page)
  const point = await duplicateCenter(page)
  await page.mouse.click(box.x + point.x, box.y + point.y)
}

const storedDocument = async (page: Page): Promise<any> =>
  page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? 'null'), STORAGE_KEY)

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
})

test('toolbar Duplicate inserts a copy right after the active circuit and activates it', async ({ page }) => {
  const before = await snapshot(page)
  const activeIndex = before.entries.findIndex((entry) => entry.id === before.active_id)
  if (activeIndex < 0) throw new Error('active circuit missing before duplicate')
  const active = before.entries[activeIndex]
  if (!active) throw new Error('active circuit missing before duplicate')

  await clickDuplicate(page)

  const after = await waitForSnapshot(page, (state) => state.entries.length === before.entries.length + 1, 'duplicate inserted')
  const inserted = after.entries[activeIndex + 1]
  const stored = await storedDocument(page)
  expect({ inserted, activeId: after.active_id, storedActiveId: stored.activeId, storedInserted: stored.circuits[activeIndex + 1] }).toMatchObject({
    inserted: { name: `${active.name} (copy)`, circuit_json: active.circuit_json },
    activeId: inserted.id,
    storedActiveId: inserted.id,
    storedInserted: { id: inserted.id, name: `${active.name} (copy)`, json: active.circuit_json },
  })
})

test('toolbar Duplicate increments copy suffixes on consecutive clicks', async ({ page }) => {
  await clickDuplicate(page)
  await clickDuplicate(page)
  await clickDuplicate(page)

  const state = await snapshot(page)
  expect({ names: state.entries.slice(0, 4).map((entry) => entry.name), activeId: state.active_id }).toEqual({
    names: ['Circuit 1', 'Circuit 1 (copy)', 'Circuit 1 (copy 2)', 'Circuit 1 (copy 3)'],
    activeId: state.entries[3].id,
  })
})

test('toolbar Duplicate exposes the Duplicate circuit tooltip on hover', async ({ page }) => {
  const box = await canvasBox(page)
  const point = await duplicateCenter(page)
  await page.locator('#egui-canvas').hover({ position: point })
  await page.waitForTimeout(200)

  let tooltip: string | undefined
  for (let attempt = 0; attempt < 50; attempt += 1) {
    tooltip = await page.evaluate(() => (window as any).__qniToolbarTooltipText)
    if (tooltip === 'Duplicate circuit') break
    await page.waitForTimeout(50)
  }
  const geometry = await duplicateGeometry(page)
  expect({ tooltip, hovered: geometry.hovered }).toEqual({ tooltip: 'Duplicate circuit', hovered: true })
})
