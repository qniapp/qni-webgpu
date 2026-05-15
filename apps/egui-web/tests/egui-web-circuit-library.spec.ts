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

const waitForEmptyLibrary = async (page: Page): Promise<LibraryDocument> => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const document = await libraryList(page)
    if (document.activeId === null && document.circuits.length === 0) return document
    await page.waitForTimeout(50)
  }
  throw new Error('localStorage circuit library did not become empty')
}

const errorMessage = async (operation: () => Promise<unknown>): Promise<string> => {
  try {
    await operation()
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
  throw new Error('operation unexpectedly succeeded')
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page)
  await libraryClear(page)
})

test('localStorage circuit library supports save, list, load, rename, and delete without UI coupling', async ({ page }) => {
  const initial = await waitForEmptyLibrary(page)

  const circuitJson = '{"cols":[["H"],["•","X"]]}'
  const id = await librarySave(page, '  Bell state  ', circuitJson)
  let document = await libraryList(page)
  const savedActiveId = document.activeId
  const saved = document.circuits[0]
  const loadedJson = await libraryLoad(page, id)
  const afterLoad = await libraryList(page)

  await libraryRename(page, id, '  Renamed Bell  ')
  document = await libraryList(page)
  const renamed = document.circuits[0]

  const qftJson = '{"cols":[["QFT3"]]}'
  const qftId = await librarySave(page, 'QFT span', qftJson)
  document = await libraryList(page)
  const qft = document.circuits[0]

  await libraryDelete(page, qftId)
  await libraryDelete(page, id)
  const final = await waitForEmptyLibrary(page)

  expect({
    initial,
    idMatches: /^ckt_\d+_[0-9a-f]{6}$/.test(id),
    savedActiveId,
    saved,
    savedTimestampOk: saved ? saved.updatedAt >= saved.createdAt : false,
    loadedJson,
    activeIdAfterLoad: afterLoad.activeId,
    renamedName: renamed?.name,
    renamedTimestampOk: renamed ? renamed.updatedAt >= renamed.createdAt : false,
    qftActiveId: qft ? document.activeId : null,
    qft,
    final,
  }).toEqual({
    initial: { version: 1, activeId: null, circuits: [] },
    idMatches: true,
    savedActiveId: id,
    saved: {
      id,
      name: 'Bell state',
      json: circuitJson,
      createdAt: saved?.createdAt,
      updatedAt: saved?.updatedAt,
      meta: { qubits: 2, columns: 2, gateCount: 3 },
    },
    savedTimestampOk: true,
    loadedJson: circuitJson,
    activeIdAfterLoad: id,
    renamedName: 'Renamed Bell',
    renamedTimestampOk: true,
    qftActiveId: qftId,
    qft: {
      id: qftId,
      name: 'QFT span',
      json: qftJson,
      createdAt: qft?.createdAt,
      updatedAt: qft?.updatedAt,
      meta: { qubits: 3, columns: 1, gateCount: 1 },
    },
    final: { version: 1, activeId: null, circuits: [] },
  })
})

test('localStorage circuit library rejects invalid names, invalid circuits, and corrupted documents', async ({ page }) => {
  const messages = [
    await errorMessage(() => librarySave(page, '   ', '{"cols":[]}')),
    await errorMessage(() => librarySave(page, 'Bad circuit', '{"bad":[]}')),
    await errorMessage(() => librarySave(page, 'Unknown gate', '{"cols":[["BAD"]]}')),
    await errorMessage(() => librarySave(page, 'Trailing garbage', '{"cols":[]} trailing')),
  ]

  await page.evaluate((key) => localStorage.setItem(key, '{not json'), STORAGE_KEY)
  messages.push(await errorMessage(() => libraryList(page)))

  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({ version: 2, circuits: [] })), STORAGE_KEY)
  messages.push(await errorMessage(() => libraryList(page)))

  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({ version: 1, activeId: null })), STORAGE_KEY)
  messages.push(await errorMessage(() => libraryList(page)))

  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({ version: 1, activeId: 'missing', circuits: [] })), STORAGE_KEY)
  messages.push(await errorMessage(() => libraryList(page)))

  await page.evaluate((key) => {
    localStorage.setItem(key, JSON.stringify({
      version: 1,
      activeId: null,
      circuits: [{ id: 'ckt_bad', name: 'Bad', json: '{"cols":[]}', createdAt: 1, updatedAt: 1 }],
    }))
  }, STORAGE_KEY)
  messages.push(await errorMessage(() => libraryList(page)))

  expect(messages.map((message) => message.replace(/^.*?(circuit|invalid|unsupported|localStorage)/, '$1'))).toEqual([
    'circuit name is empty',
    'invalid circuit json',
    'invalid circuit json',
    'invalid circuit json',
    'circuit library is corrupted',
    'unsupported circuit library version',
    'circuit library is corrupted',
    'circuit library is corrupted',
    'circuit library is corrupted',
  ])
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

  let message: string
  try {
    message = await errorMessage(() => librarySave(page, 'Too large', '{"cols":[]}'))
  } finally {
    await page.evaluate(() => (window as any).__restoreQniStorageSetItem?.())
  }

  expect({
    quotaError: message.includes('localStorage error: QuotaExceededError'),
    document: await waitForEmptyLibrary(page),
  }).toEqual({
    quotaError: true,
    document: { version: 1, activeId: null, circuits: [] },
  })
})
