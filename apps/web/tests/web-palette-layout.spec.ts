import { expect, test, type Page } from '@playwright/test'
import {
  UI_CONSTANTS,
  dragPointer,
  getPaletteGateCenter,
  getCircuitCellCenter,
  pollForValue,
  waitForStartupReady,
  waitForValue,
  type Point,
} from './support/web-spec-helpers'

type Layout = { cells: Point[][] }

const EGUI_PANEL_MARGIN = 8
const PALETTE_ENTRY_COUNT = 39
const EMPTY = '#%7B%22cols%22%3A%5B%5D%7D'
const BELL = `#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['•', 'X'], ['Measure', 'Measure']] }))}`

const open = async (page: Page, hash: string): Promise<Layout> => {
  await page.goto('/' + hash)
  await waitForStartupReady(page, { waitForStateVector: true })
  return waitForValue(
    async () => JSON.parse(await page.evaluate(() => String(Reflect.get(window, '__qniLayoutJson') ?? 'null'))) as Layout | null,
    (layout) => (layout?.cells.length ?? 0) > 0,
    'circuit geometry was not published',
  ) as Promise<Layout>
}

const hoveredPaletteIndex = async (page: Page, at: Point, expected: number): Promise<number | null> => {
  await page.mouse.move(at.x, at.y)
  return pollForValue(
    async () => JSON.parse(await page.evaluate(() => String(Reflect.get(window, '__qniHoverSnapshotJson') ?? 'null')))?.hoveredPaletteIndex ?? null,
    (index) => index === expected,
    { timeout: 2_000 },
  )
}

const paletteCenter = (width: number, index: number): Point => {
  const center = getPaletteGateCenter(width, index)
  return { x: center.x, y: center.y + EGUI_PANEL_MARGIN }
}

for (const viewport of [
  { width: 1280, height: 800 },
  { width: 1000, height: 800 },
  { width: 900, height: 800 },
  { width: 840, height: 800 },
  { width: 800, height: 800 },
]) {
  test.describe(`${viewport.width}x${viewport.height}`, () => {
    test.use({ viewport })

    test('every palette gate centre hovers that gate', async ({ page }) => {
      await open(page, EMPTY)
      const hovered: (number | null)[] = []
      for (let index = 0; index < PALETTE_ENTRY_COUNT; index += 1) {
        hovered.push(await hoveredPaletteIndex(page, paletteCenter(viewport.width, index), index))
      }
      expect(hovered).toEqual(Array.from({ length: PALETTE_ENTRY_COUNT }, (_, index) => index))
    })

    test('a palette gate drops onto the first wire below the palette', async ({ page }) => {
      const layout = await open(page, EMPTY)
      await dragPointer(page, paletteCenter(viewport.width, 0), layout.cells[0][0])
      await expect.poll(() => JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1))).cols).toEqual([['H']])
    })
  })
}

for (const width of [1000, 1024]) {
  test.describe(`1440 to ${width}px three-row palette`, () => {
    test.beforeEach(async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 800 })
      await open(page, EMPTY)
      await page.setViewportSize({ width, height: 800 })
      const last = paletteCenter(width, PALETTE_ENTRY_COUNT - 1)
      await page.mouse.move(last.x, last.y)
      await waitForValue(
        () => page.evaluate(() => JSON.parse(String(Reflect.get(window, '__qniHoverSnapshotJson') ?? 'null'))?.hoveredPaletteIndex),
        index => index === PALETTE_ENTRY_COUNT - 1,
        'resized palette did not reach its final item',
      )
    })

    test('every palette item remains reachable after shrinking', async ({ page }) => {
      const hovered: (number | null)[] = []
      for (let index = 0; index < PALETTE_ENTRY_COUNT; index += 1) {
        hovered.push(await hoveredPaletteIndex(page, paletteCenter(width, index), index))
      }
      expect(hovered).toEqual(Array.from({ length: PALETTE_ENTRY_COUNT }, (_, index) => index))
    })

    test('toolbar stays inside the viewport above the palette after shrinking', async ({ page }) => {
      const toolbar = await page.evaluate(() =>
        ['Picker', 'Undo', 'Redo', 'Clear', 'Duplicate', 'Lock', 'Tutorial']
          .map(name => Reflect.get(window, `__qniToolbar${name}GeometryJson`))
          .map(raw => typeof raw === 'string' && raw.length > 0
            ? JSON.parse(raw) as { left: number; right: number; top: number; bottom: number }
            : null),
      )
      const paletteTop = paletteCenter(width, 0).y - UI_CONSTANTS.PALETTE_SIZE / 2 - UI_CONSTANTS.PALETTE_PADDING_Y
      expect(toolbar.every(rect => rect !== null && rect.left >= 0 && rect.right <= width
        && rect.top >= 0 && rect.bottom < paletteTop)).toBe(true)
    })

    test('circuit shifts down for the third palette row after shrinking', async ({ page }) => {
      const layout = await page.evaluate(() => JSON.parse(String(Reflect.get(window, '__qniLayoutJson'))) as Layout)
      expect(layout.cells[0][0].y).toBe(getCircuitCellCenter(width).y)
    })
  })
}

test.describe('1000x800 screenshot', () => {
  test.use({ viewport: { width: 1000, height: 800 } })

  test('palette fits in three rows with the Display section beside the top gate rows', async ({ page }) => {
    await open(page, BELL)
    await expect(page).toHaveScreenshot('palette-1000x800.png')
  })
})

test.describe('800x800 screenshot', () => {
  test.use({ viewport: { width: 800, height: 800 } })

  test('palette wraps into four rows above the circuit', async ({ page }) => {
    await open(page, BELL)
    await expect(page).toHaveScreenshot('palette-800x800.png')
  })
})
