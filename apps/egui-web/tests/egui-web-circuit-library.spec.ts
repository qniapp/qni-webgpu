import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady } from './support/egui-web-spec-helpers'

const STORAGE_KEY = 'qni.circuit_library.v1'

type LibraryDocument = {
  version: number
  activeId: string | null
  circuits: Array<{
    id: string
    name: string
    json: string
    createdAt: number
    updatedAt: number
    meta: { qubits: number; columns: number; gateCount: number }
  }>
}

const libraryList = async (page: Page): Promise<LibraryDocument> => {
  const raw = await page.evaluate(() => {
    const fn = (window as any).__qniCircuitLibraryList
    if (typeof fn !== 'function') {
      throw new Error('__qniCircuitLibraryList hook missing')
    }
    return fn()
  })
  return JSON.parse(raw) as LibraryDocument
}

const librarySave = async (page: Page, name: string, circuitJson: string): Promise<string> =>
  page.evaluate(({ name: n, circuitJson: json }) => {
    const fn = (window as any).__qniCircuitLibrarySave
    if (typeof fn !== 'function') {
      throw new Error('__qniCircuitLibrarySave hook missing')
    }
    return fn(n, json)
  }, { name, circuitJson })

const libraryLoad = async (page: Page, id: string): Promise<string> =>
  page.evaluate((entryId) => {
    const fn = (window as any).__qniCircuitLibraryLoad
    if (typeof fn !== 'function') {
      throw new Error('__qniCircuitLibraryLoad hook missing')
    }
    return fn(entryId)
  }, id)

const libraryRename = async (page: Page, id: string, name: string): Promise<void> =>
  page.evaluate(({ entryId, nextName }) => {
    const fn = (window as any).__qniCircuitLibraryRename
    if (typeof fn !== 'function') {
      throw new Error('__qniCircuitLibraryRename hook missing')
    }
    fn(entryId, nextName)
  }, { entryId: id, nextName: name })

const libraryDelete = async (page: Page, id: string): Promise<void> =>
  page.evaluate((entryId) => {
    const fn = (window as any).__qniCircuitLibraryDelete
    if (typeof fn !== 'function') {
      throw new Error('__qniCircuitLibraryDelete hook missing')
    }
    fn(entryId)
  }, id)

const libraryClear = async (page: Page): Promise<void> =>
  page.evaluate(() => {
    const fn = (window as any).__qniCircuitLibraryClear
    if (typeof fn !== 'function') {
      throw new Error('__qniCircuitLibraryClear hook missing')
    }
    fn()
  })

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page)
  await libraryClear(page)
})

test('localStorage circuit library supports save, list, load, rename, and delete without UI coupling', async ({ page }) => {
  await expect.poll(async () => await libraryList(page)).toEqual({
    version: 1,
    activeId: null,
    circuits: [],
  })

  const circuitJson = '{"cols":[["H"],["•","X"]]}'
  const id = await librarySave(page, '  Bell state  ', circuitJson)
  expect(id).toMatch(/^ckt_\d+_[0-9a-f]{6}$/)

  let document = await libraryList(page)
  expect(document.activeId).toBe(id)
  expect(document.circuits).toHaveLength(1)
  expect(document.circuits[0]).toMatchObject({
    id,
    name: 'Bell state',
    json: circuitJson,
    meta: { qubits: 2, columns: 2, gateCount: 3 },
  })
  expect(document.circuits[0].updatedAt).toBeGreaterThanOrEqual(document.circuits[0].createdAt)

  await expect(libraryLoad(page, id)).resolves.toBe(circuitJson)
  document = await libraryList(page)
  expect(document.activeId).toBe(id)

  await libraryRename(page, id, '  Renamed Bell  ')
  document = await libraryList(page)
  expect(document.circuits[0].name).toBe('Renamed Bell')
  expect(document.circuits[0].updatedAt).toBeGreaterThanOrEqual(document.circuits[0].createdAt)

  const qftId = await librarySave(page, 'QFT span', '{"cols":[["QFT3"]]}')
  document = await libraryList(page)
  expect(document.activeId).toBe(qftId)
  expect(document.circuits[0]).toMatchObject({
    id: qftId,
    meta: { qubits: 3, columns: 1, gateCount: 1 },
  })

  await libraryDelete(page, qftId)
  await libraryDelete(page, id)
  await expect.poll(async () => await libraryList(page)).toEqual({
    version: 1,
    activeId: null,
    circuits: [],
  })
})

test('localStorage circuit library rejects invalid names, invalid circuits, and corrupted documents', async ({ page }) => {
  await expect(librarySave(page, '   ', '{"cols":[]}')).rejects.toThrow(/circuit name is empty/)
  await expect(librarySave(page, 'Bad circuit', '{"bad":[]}')).rejects.toThrow(/invalid circuit json/)
  await expect(librarySave(page, 'Unknown gate', '{"cols":[["BAD"]]}')).rejects.toThrow(/invalid circuit json/)
  await expect(librarySave(page, 'Trailing garbage', '{"cols":[]} trailing')).rejects.toThrow(/invalid circuit json/)

  await page.evaluate((key) => localStorage.setItem(key, '{not json'), STORAGE_KEY)
  await expect(libraryList(page)).rejects.toThrow(/circuit library is corrupted/)

  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({ version: 2, circuits: [] })), STORAGE_KEY)
  await expect(libraryList(page)).rejects.toThrow(/unsupported circuit library version/)

  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({ version: 1, activeId: null })), STORAGE_KEY)
  await expect(libraryList(page)).rejects.toThrow(/circuit library is corrupted/)

  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({ version: 1, activeId: 'missing', circuits: [] })), STORAGE_KEY)
  await expect(libraryList(page)).rejects.toThrow(/circuit library is corrupted/)

  await page.evaluate((key) => {
    localStorage.setItem(key, JSON.stringify({
      version: 1,
      activeId: null,
      circuits: [{ id: 'ckt_bad', name: 'Bad', json: '{"cols":[]}', createdAt: 1, updatedAt: 1 }],
    }))
  }, STORAGE_KEY)
  await expect(libraryList(page)).rejects.toThrow(/circuit library is corrupted/)
})

test('localStorage circuit library reports quota errors without mutating the document', async ({ page }) => {
  await page.evaluate((key) => {
    const originalSetItem = Storage.prototype.setItem
    ;(window as any).__restoreQniStorageSetItem = () => {
      Storage.prototype.setItem = originalSetItem
    }
    Storage.prototype.setItem = function patchedSetItem(name: string, value: string): void {
      if (name === key) {
        throw new DOMException('quota exceeded', 'QuotaExceededError')
      }
      originalSetItem.call(this, name, value)
    }
  }, STORAGE_KEY)

  try {
    await expect(librarySave(page, 'Too large', '{"cols":[]}')).rejects.toThrow(/localStorage error: QuotaExceededError/)
  } finally {
    await page.evaluate(() => (window as any).__restoreQniStorageSetItem?.())
  }

  await expect.poll(async () => await libraryList(page)).toEqual({
    version: 1,
    activeId: null,
    circuits: [],
  })
})
