import { expect, test } from '@playwright/test'
import { readStateVector, waitForStartupReady, waitForValue } from './support/web-spec-helpers'

const circuit = '#%7B%22cols%22%3A%5B%5B%22H%22%5D%2C%5B%22Z%22%5D%5D%7D'
const firstStep = [Math.SQRT1_2, 0, Math.SQRT1_2, 0]
const matches = (actual: unknown[], expected: number[]) =>
  actual.length === expected.length && expected.every((value, index) => Math.abs(Number(actual[index]) - value) < 1e-3)

test('standalone circuit initially shows the first column state', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  expect(matches(await readStateVector(page), firstStep)).toBe(true)
})

test('hovering above the first gate previews step zero', async ({ page }) => {
  await page.goto('/' + circuit)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.click(226, 304)
  await page.mouse.move(1000, 450)
  await waitForValue(() => readStateVector(page), state => matches(state, [Math.SQRT1_2, 0, -Math.SQRT1_2, 0]), 'Z column was not selected')
  await page.mouse.move(170, 224)
  const preview = await waitForValue(() => readStateVector(page), state => matches(state, firstStep), 'step-zero hover preview did not appear')
  expect(matches(preview, firstStep)).toBe(true)
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
  const selected = await waitForValue(() => readStateVector(page), state => matches(state, firstStep), 'step-zero click was not selected')
  expect(matches(selected, firstStep)).toBe(true)
})
