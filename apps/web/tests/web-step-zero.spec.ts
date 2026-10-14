import { expect, test } from '@playwright/test'
import { readStateVector, waitForStartupReady, waitForValue } from './support/web-spec-helpers'

const circuit = '#%7B%22cols%22%3A%5B%5B%22H%22%5D%2C%5B%22Z%22%5D%5D%7D'
const firstStep = [Math.SQRT1_2, 0, Math.SQRT1_2, 0]
const finalStep = [Math.SQRT1_2, 0, -Math.SQRT1_2, 0]
const matches = (actual: unknown[], expected: number[]) =>
  actual.length === expected.length && expected.every((value, index) => Math.abs(Number(actual[index]) - value) < 1e-3)

test('standalone circuit initially shows the final column state', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await expect.poll(async () => matches(await readStateVector(page), finalStep)).toBe(true)
})

test('hovering above the first gate previews step zero', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(226, 304)
  await page.mouse.move(1000, 450)
  await waitForValue(() => readStateVector(page), state => matches(state, [Math.SQRT1_2, 0, -Math.SQRT1_2, 0]), 'Z column was not selected')
  await page.mouse.move(170, 224)
  await expect.poll(async () => matches(await readStateVector(page), firstStep)).toBe(true)
})

test('clicking above the first gate selects step zero', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  // Change the preview/selection to the Z column, then leave it before clicking.
  await page.mouse.click(226, 304)
  await page.mouse.move(1000, 450)
  await waitForValue(() => readStateVector(page), state => matches(state, [Math.SQRT1_2, 0, -Math.SQRT1_2, 0]), 'Z column was not selected')
  await page.mouse.click(170, 224) // 32px above q0: inside qni's step padding, outside the gate.
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), firstStep)).toBe(true)
})

const initialState = [1, 0, 0, 0]

test('hovering the left gap previews the initial state then restores the final state', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.move(132, 224)
  await waitForValue(() => readStateVector(page), state => matches(state, initialState), 'initial hover did not show ground state')
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), finalStep)).toBe(true)
})

test('initial hover reports minus one to the inspection hook', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.move(132, 224)
  await expect.poll(() => page.evaluate(() =>
    JSON.parse((window as any).__qniHoverSnapshotJson).hoveredStep)).toBe(-1)
})

test('clicking the left gap locks the initial state after pointer leaves', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})

test('clicking the middle of the gap pins initial state after pointer leaves', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(140, 224)
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})

test('clicking the extended left end pins initial state', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(100, 224)
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})

test('clicking just before the first gate left edge pins initial state', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(149, 224)
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})

test('clicking just inside the first gate left edge selects column zero', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.click(151, 224) // Above the gate body, in column zero's step padding.
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), firstStep)).toBe(true)
})

test('hovering post-H temporarily overrides the initial breakpoint', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.move(170, 224)
  await expect.poll(async () => matches(await readStateVector(page), firstStep)).toBe(true)
})

test('dragging a gate overrides the initial breakpoint with the tentative final state', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.move(170, 264)
  await page.mouse.down()
  await page.mouse.move(282, 264, { steps: 8 })
  await expect.poll(async () => matches(await readStateVector(page), [Math.SQRT1_2, 0, Math.SQRT1_2, 0])).toBe(true)
  await page.keyboard.press('Escape')
  await page.mouse.up()
})

test('initial breakpoint is drawn in the existing gap without moving gates', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.move(1000, 450)
  await waitForValue(() => readStateVector(page), state => matches(state, initialState), 'initial selection not ready')
  await expect(page).toHaveScreenshot('initial-breakpoint.png', {
    clip: { x: 80, y: 200, width: 200, height: 128 },
  })
})

test('hovering the left gap draws a translucent initial bar', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(170, 224) // 既存の比較画像と同じ列 0 を固定してからホバーする。
  await page.mouse.move(132, 224)
  await waitForValue(() => readStateVector(page), state => matches(state, initialState), 'initial hover not ready')
  await expect(page).toHaveScreenshot('initial-hover.png', {
    clip: { x: 80, y: 200, width: 200, height: 128 },
  })
})

test('narrow circuit keeps the initial hit target without shifting columns', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 })
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(52, 224)
  await page.mouse.move(389, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})

test('narrow circuit hits the extended left end', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 })
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(25, 224)
  await page.mouse.move(389, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})

test('dragging into the initial bar still inserts before column zero', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.move(226, 264)
  await page.mouse.down()
  await page.mouse.move(140, 264, { steps: 12 })
  await page.mouse.up()
  await page.mouse.move(1000, 450)
  await expect.poll(() => JSON.parse(decodeURIComponent(page.url().split('#')[1] ?? '')).cols).toEqual([['Z'], ['H']])
})

test('initial breakpoint is restored after a tentative gate drag', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(132, 224)
  await page.mouse.move(170, 264)
  await page.mouse.down()
  await page.mouse.move(282, 264, { steps: 8 })
  await waitForValue(() => readStateVector(page), state => matches(state, firstStep), 'drag did not show tentative final state')
  await page.mouse.up()
  await page.mouse.move(1000, 450)
  await expect.poll(async () => matches(await readStateVector(page), initialState)).toBe(true)
})
