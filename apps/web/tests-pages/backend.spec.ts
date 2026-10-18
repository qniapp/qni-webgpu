import { expect, test } from '@playwright/test'
import { readStateVector, sampleCanvasPixels, waitForStartupReady } from '../tests/support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

const hash = encodeURIComponent(JSON.stringify({ cols: [['X']] }))
const oversizedCols = [[...Array(16).fill(1), 'H']]
const oversizedHash = encodeURIComponent(JSON.stringify({ cols: oversizedCols }))

test('Pages ignores the GPU URL option and computes locally', async ({ page }) => {
  await page.goto(`./?mode=gpu#${hash}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const state = await readStateVector(page)
  expect(state.slice(0, 4)).toEqual([0, 0, 1, 0])
})

test('Pages removes the GPU URL option', async ({ page }) => {
  await page.goto(`./?mode=gpu#${hash}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  expect(new URL(page.url()).searchParams.has('mode')).toBe(false)
})

test('Pages leaves the former toggle area empty', async ({ page }) => {
  await page.goto(`./#${hash}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'formerLocal', x: 1180, y: 23 },
    { name: 'toolbar', x: 1000, y: 23 },
  ])
  expect(pixels.formerLocal).toEqual(pixels.toolbar)
})

test('Pages renders the seventeenth wire of an oversized shared circuit', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 1400 })
  await page.goto(`./#${oversizedHash}`)
  await waitForStartupReady(page)
  const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [
    { name: 'wire', x: 300, y: 1160 },
    { name: 'background', x: 300, y: 1180 },
  ])
  expect(pixels.wire).not.toEqual(pixels.background)
})

test('Pages preserves every gate of an oversized shared circuit', async ({ page }) => {
  await page.goto(`./#${oversizedHash}`)
  await waitForStartupReady(page)
  const circuit = JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1)))
  expect(circuit.cols).toEqual(oversizedCols)
})

test('Pages never auto-selects GPU for an oversized shared circuit', async ({ page }) => {
  await page.goto(`./?mode=gpu#${oversizedHash}`)
  await waitForStartupReady(page)
  expect(new URL(page.url()).searchParams.has('mode')).toBe(false)
})
