import { expect, test, type Page } from '@playwright/test'
import { readStateVector, waitForStartupReady, waitForValue } from './support/web-spec-helpers'
import { collectBrowserErrors } from '../test-support/browser-errors'
import { writeFile } from 'node:fs/promises'

type Rect = { left: number; top: number; right: number; bottom: number }
type Point = { x: number; y: number }
type Carousel = {
  rect: Rect
  page: number
  pageCount: number
  slots: Rect[]
  prev: Rect
  next: Rect
  dots: Point[]
}
type Layout = {
  orientation: 'horizontal' | 'vertical'
  circuitRect: Rect
  cells: Point[][]
  carousel: Carousel | null
}

const PHONE = { width: 390, height: 844 }
const EMPTY = '#%7B%22cols%22%3A%5B%5D%7D'
const BELL = `#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['•', 'X'], ['Measure', 'Measure']] }))}`
const POSTSELECTION = `#${encodeURIComponent(JSON.stringify({ cols: [['H', 'H'], ['|0⟩⟨0|'], ['H'], ['|+⟩⟨+|']] }))}`
const LONG = `#${encodeURIComponent(JSON.stringify({ cols: Array.from({ length: 12 }, () => ['H']) }))}`

test.use({ viewport: PHONE, hasTouch: true, isMobile: true, deviceScaleFactor: 1 })

const readLayout = async (page: Page): Promise<Layout> =>
  JSON.parse(await page.evaluate(() => String(Reflect.get(window, '__qniLayoutJson') ?? 'null')))

const openPhone = async (page: Page, hash: string): Promise<Layout> => {
  await page.goto('/' + hash)
  await waitForStartupReady(page, { waitForStateVector: true })
  return waitForValue(() => readLayout(page), layout => layout?.carousel != null, 'vertical layout geometry was not published')
}

const center = (rect: Rect): Point => ({ x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 })

const carouselOf = (layout: Layout): Carousel => {
  if (!layout.carousel) throw new Error('palette carousel is not shown')
  return layout.carousel
}

const readPage = async (page: Page): Promise<number> => carouselOf(await readLayout(page)).page

const readHashCols = (page: Page): unknown =>
  JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1))).cols

const mouseDrag = async (page: Page, from: Point, to: Point): Promise<void> => {
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  await page.waitForTimeout(32)
  await page.mouse.move(to.x, to.y, { steps: 8 })
  await page.waitForTimeout(32)
  await page.mouse.up()
}

const touchDrag = async (page: Page, from: Point, to: Point, end: 'touchEnd' | 'touchCancel' = 'touchEnd'): Promise<void> => {
  const client = await page.context().newCDPSession(page)
  await client.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [from] })
  await page.waitForTimeout(32)
  for (let step = 1; step <= 8; step += 1) {
    const t = step / 8
    await client.send('Input.dispatchTouchEvent', {
      type: 'touchMove',
      touchPoints: [{ x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t }],
    })
    await page.waitForTimeout(16)
  }
  await page.waitForTimeout(32)
  await client.send('Input.dispatchTouchEvent', { type: end, touchPoints: [] })
  await client.detach()
}

const amplitudes = (state: unknown[]): number[] => state.map(Number)

const matches = (actual: number[], expected: number[]): boolean =>
  actual.length === expected.length && expected.every((value, index) => Math.abs(actual[index] - value) < 1e-3)

test('phone portrait uses the vertical layout', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  expect(layout.orientation).toBe('vertical')
})

test('palette carousel starts on the first of ten pages', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  expect({ page: carousel.page, pageCount: carousel.pageCount }).toEqual({ page: 0, pageCount: 10 })
})

test('palette carousel sits below the circuit area', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  expect(carouselOf(layout).rect.top).toBeGreaterThan(layout.circuitRect.bottom)
})

test('wire 0 is the rightmost wire', async ({ page }) => {
  const layout = await openPhone(page, BELL)
  expect(layout.cells[0][0].x).toBeGreaterThan(layout.cells[0][1].x)
})

test('time runs down the screen', async ({ page }) => {
  const layout = await openPhone(page, BELL)
  expect(layout.cells[1][0].y).toBeGreaterThan(layout.cells[0][0].y)
})

test('tapping the next arrow shows the second palette page', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  const next = center(carousel.next)
  await page.touchscreen.tap(next.x, next.y)
  await page.waitForTimeout(200)
  expect(await readPage(page)).toBe(1)
})

test('tapping the previous arrow on the first page stays there', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  const prev = center(carousel.prev)
  await page.touchscreen.tap(prev.x, prev.y)
  await page.waitForTimeout(200)
  expect(await readPage(page)).toBe(0)
})

test('tapping the next arrow on the last page stays there', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  const last = carousel.dots[carousel.pageCount - 1]
  await page.touchscreen.tap(last.x, last.y)
  await waitForValue(() => readPage(page), current => current === carousel.pageCount - 1, 'last page was not shown')
  const next = center(carousel.next)
  await page.touchscreen.tap(next.x, next.y)
  await page.waitForTimeout(200)
  expect(await readPage(page)).toBe(carousel.pageCount - 1)
})

test('tapping a page dot jumps to that page', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  await page.touchscreen.tap(carousel.dots[5].x, carousel.dots[5].y)
  await expect.poll(() => readPage(page)).toBe(5)
})

test('touch drag of the last carousel item creates a comment block', async ({ page }) => {
  const layout = await openPhone(page, `#${encodeURIComponent('{"cols":[["H"]]}')}`)
  const carousel = carouselOf(layout)
  const lastPage = carousel.pageCount - 1
  await page.touchscreen.tap(carousel.dots[lastPage].x, carousel.dots[lastPage].y)
  await waitForValue(() => readPage(page), current => current === lastPage, 'last page was not shown')
  await touchDrag(page, center(carousel.slots[2]), layout.cells[0][0])

  await expect.poll(() => readHashCols(page)).toEqual([['{comment'], ['H'], ['}']])
})

test('portrait comment label opens the upright editor and commits text', async ({ page }) => {
  await openPhone(page, `#${encodeURIComponent('{"cols":[["{comment"],["H"],["}"]]}')}`)
  const geometry = await waitForValue(
    () => page.evaluate(() => JSON.parse(String(Reflect.get(window, '__qniCircuitBlockGeometryJson') ?? 'null'))),
    value => value?.blocks.length === 1,
    'comment geometry was not published',
  )
  const label = center(geometry.blocks[0].labels[0])
  await page.touchscreen.tap(label.x, label.y)
  await page.waitForFunction(() => document.activeElement?.tagName === 'INPUT')
  await page.keyboard.type('portrait')
  await page.keyboard.press('Enter')

  await expect.poll(() => readHashCols(page)).toEqual([['{portrait'], ['H'], ['}']])
})

test('mouse drag of H from the palette onto wire 0 places it', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  await mouseDrag(page, center(carouselOf(layout).slots[0]), layout.cells[0][0])
  await expect.poll(() => readHashCols(page)).toEqual([['H']])
})

test('touch drag of H from the palette onto wire 0 places it', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  await touchDrag(page, center(carouselOf(layout).slots[0]), layout.cells[0][0])
  await expect.poll(() => readHashCols(page)).toEqual([['H']])
})

test('touch drag of X from the palette onto wire 1 places it there', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  await touchDrag(page, center(carouselOf(layout).slots[1]), layout.cells[0][1])
  await expect.poll(() => readHashCols(page)).toEqual([[1, 'X']])
})

for (const { name, token, slot } of [
  { name: 'increment', token: 'inc1', slot: 1 },
  { name: 'decrement', token: 'dec1', slot: 2 },
]) {
  test(`touch drag of ${name} from the fourth palette page places it`, async ({ page }) => {
    const layout = await openPhone(page, EMPTY)
    const carousel = carouselOf(layout)
    await page.touchscreen.tap(carousel.dots[3].x, carousel.dots[3].y)
    await waitForValue(() => readPage(page), current => current === 3, 'fourth page was not shown')
    await touchDrag(page, center(carousel.slots[slot]), layout.cells[0][0])

    await expect.poll(() => readHashCols(page)).toEqual([[token]])
  })

  test(`clicking ${name} in the palette without dragging does not place it`, async ({ page }) => {
    const layout = await openPhone(page, EMPTY)
    const carousel = carouselOf(layout)
    await page.touchscreen.tap(carousel.dots[3].x, carousel.dots[3].y)
    await waitForValue(() => readPage(page), current => current === 3, 'fourth page was not shown')
    const item = center(carousel.slots[slot])
    await page.mouse.click(item.x, item.y)
    await page.waitForTimeout(200)

    expect(readHashCols(page)).toEqual([])
  })
}

test('dropped H updates the state vector through the GPU path', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  await mouseDrag(page, center(carouselOf(layout).slots[0]), layout.cells[0][0])
  await expect
    .poll(async () => matches(amplitudes(await readStateVector(page)), [Math.SQRT1_2, 0, Math.SQRT1_2, 0]))
    .toBe(true)
})

test('dragging a placed gate off the circuit removes it', async ({ page }) => {
  const layout = await openPhone(page, '#%7B%22cols%22%3A%5B%5B%22H%22%5D%5D%7D')
  await mouseDrag(page, layout.cells[0][0], { x: 40, y: layout.cells[0][0].y })
  await expect.poll(() => readHashCols(page)).toEqual([])
})

test('tapping a step selects it on release', async ({ page }) => {
  const layout = await openPhone(page, '#%7B%22cols%22%3A%5B%5B%22H%22%5D%2C%5B%22Z%22%5D%5D%7D')
  const step0 = { x: layout.cells[0][0].x - 40, y: layout.cells[0][0].y }
  await page.touchscreen.tap(step0.x, step0.y)
  await expect
    .poll(async () => matches(amplitudes(await readStateVector(page)), [Math.SQRT1_2, 0, Math.SQRT1_2, 0]))
    .toBe(true)
})

test('swiping up on empty space pans the circuit along time', async ({ page }) => {
  const layout = await openPhone(page, LONG)
  const last = layout.cells.length - 1
  await touchDrag(page, { x: 60, y: layout.circuitRect.bottom - 40 }, { x: 60, y: layout.circuitRect.top + 40 })
  await expect.poll(async () => (await readLayout(page)).cells[last][0].y).toBeLessThan(layout.circuitRect.bottom)
})

test('standalone touch pan and cancellation have no console errors or interventions', async ({ page }, testInfo) => {
  const { messages, cdp } = await collectBrowserErrors(page)
  const layout = await openPhone(page, LONG)
  await touchDrag(page, { x: 60, y: layout.circuitRect.bottom - 40 }, { x: 60, y: layout.circuitRect.top + 40 })
  await touchDrag(page, { x: 60, y: layout.circuitRect.top + 40 }, { x: 60, y: layout.circuitRect.top + 40 }, 'touchCancel')
  await writeFile(testInfo.outputPath('touch-browser-log.json'), JSON.stringify(messages, null, 2))
  await testInfo.attach('touch-browser-log', { path: testInfo.outputPath('touch-browser-log.json'), contentType: 'application/json' })
  await cdp.detach()
  expect(messages).toEqual([])
})

test('standalone palette remains usable after explicit touch cancellation', async ({ page }) => {
  const layout = await openPhone(page, EMPTY)
  await touchDrag(page, { x: 60, y: layout.circuitRect.top + 40 }, { x: 60, y: layout.circuitRect.top + 40 }, 'touchCancel')
  const next = center(carouselOf(layout).next)
  await page.touchscreen.tap(next.x, next.y)
  await expect.poll(() => readPage(page)).toBe(1)
})

test('swiping across a step does not select it', async ({ page }) => {
  const layout = await openPhone(page, LONG)
  const from = { x: layout.cells[0][0].x - 40, y: layout.cells[2][0].y }
  await touchDrag(page, from, { x: from.x, y: from.y - 120 })
  await page.waitForTimeout(200)
  const amplitudesNow = amplitudes(await readStateVector(page))
  expect(matches(amplitudesNow, [1, 0, 0, 0])).toBe(true)
})

test('empty circuit renders the portrait layout', async ({ page }) => {
  await openPhone(page, EMPTY)
  await expect(page).toHaveScreenshot('mobile-empty-390x844.png')
})

test('bell circuit renders vertically', async ({ page }) => {
  await openPhone(page, BELL)
  await expect(page).toHaveScreenshot('mobile-bell-390x844.png')
})

test('survival labels sit beside the vertical circuit', async ({ page }) => {
  await openPhone(page, POSTSELECTION)
  await expect(page).toHaveScreenshot('mobile-postselection-390x844.png')
})

test('second palette page renders in the carousel', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  const next = center(carousel.next)
  await page.touchscreen.tap(next.x, next.y)
  await waitForValue(() => readPage(page), current => current === 1, 'second page was not shown')
  await expect(page).toHaveScreenshot('mobile-palette-page2-390x844.png')
})

test('fourth palette page renders arithmetic after RZ', async ({ page }) => {
  const carousel = carouselOf(await openPhone(page, EMPTY))
  await page.touchscreen.tap(carousel.dots[3].x, carousel.dots[3].y)
  await waitForValue(() => readPage(page), current => current === 3, 'fourth page was not shown')
  await expect(page).toHaveScreenshot('mobile-palette-page4-390x844.png')
})

test.describe('phone landscape', () => {
  test.use({ viewport: { width: PHONE.height, height: PHONE.width } })

  test('keeps the horizontal layout', async ({ page }) => {
    await page.goto('/' + EMPTY)
    await waitForStartupReady(page)
    const layout = await waitForValue(() => readLayout(page), value => value != null, 'layout geometry was not published')
    expect(layout.orientation).toBe('horizontal')
  })
})

test('dragging the portrait comment label along time moves rather than deletes the block', async ({ page }) => {
  const layout = await openPhone(page, `#${encodeURIComponent('{"cols":[["{comment"],["H"],["}"],["X"]]}')}`)
  const geometry = await waitForValue(
    () => page.evaluate(() => JSON.parse(String(Reflect.get(window, '__qniCircuitBlockGeometryJson') ?? 'null'))),
    value => value?.blocks.length === 1,
    'comment geometry was not published',
  )
  const label = center(geometry.blocks[0].labels[0])
  await touchDrag(page, label, { x: label.x, y: label.y + layout.cells[1][0].y - layout.cells[0][0].y })

  await expect.poll(() => readHashCols(page)).toEqual([['H'], ['{comment'], ['X'], ['}']])
})
