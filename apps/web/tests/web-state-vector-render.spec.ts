import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  readStateVector,
  sampleCanvasPixels,
  UI_CONSTANTS,
  waitForStartupReady,
  type CanvasPixel,
  type Point,
} from './support/web-spec-helpers'

const FLEXOKI_BLUE_400: CanvasPixel = [67, 133, 190, 255] // Flexoki blue-400 #4385BE.

const defaultFilledStateCircleCenter = (cssWidth: number, cssHeight: number): Point => {
  const EGUI_PANEL_MARGIN = 8
  const STATE_PANEL_WIDTH = UI_CONSTANTS.STATE_VIEWPORT_DEFAULT_WIDTH
  const STATE_VIEWPORT_HEIGHT = UI_CONSTANTS.STATE_VIEWPORT_DEFAULT_HEIGHT
  const STATE_HANDLE_HEIGHT = UI_CONSTANTS.STATE_HANDLE_HEIGHT
  const STATE_BOTTOM_MARGIN = UI_CONSTANTS.STATE_CIRCLE_BOTTOM_MARGIN
  const innerWidth = cssWidth - EGUI_PANEL_MARGIN * 2
  const innerHeight = cssHeight - EGUI_PANEL_MARGIN * 2
  const stateRectMinX = EGUI_PANEL_MARGIN + innerWidth / 2 - STATE_PANEL_WIDTH / 2
  const stateRectMinY =
    EGUI_PANEL_MARGIN + innerHeight - STATE_BOTTOM_MARGIN - STATE_VIEWPORT_HEIGHT - STATE_HANDLE_HEIGHT
  const viewportMinY = stateRectMinY + STATE_HANDLE_HEIGHT
  const circleSize = 64
  const circleGap = 3
  const gridWidth = circleSize * 2 + circleGap
  const gridOriginX = stateRectMinX + (STATE_PANEL_WIDTH - gridWidth) / 2
  const gridOriginY = viewportMinY + (STATE_VIEWPORT_HEIGHT - circleSize) / 2

  return { x: gridOriginX + circleSize / 2, y: gridOriginY + circleSize / 2 }
}

async function secondCirclePixels(page: Page) {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const first = defaultFilledStateCircleCenter(box.width, box.height)
  return sampleCanvasPixels(page, canvas, [
    { name: 'needle', x: first.x + 67, y: first.y - 16 },
    { name: 'outline', x: first.x + 67 + 32, y: first.y },
  ])
}

test('state vector renders sub-micro amplitudes like zero, without a phase needle', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })
  const zero = await secondCirclePixels(page)
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['Ry(π/134217728)']] })))
  await page.reload()
  await waitForStartupReady(page, { waitForStateVector: true })
  expect(await secondCirclePixels(page)).toEqual(zero)
})

test('state vector retains sub-micro amplitudes in GPU simulation', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['Ry(π/134217728)']] })))
  await waitForStartupReady(page, { waitForStateVector: true })
  expect((await readStateVector(page))[2]).toBeCloseTo(Math.PI / 268435456, 14)
})

test('state vector keeps the phase needle above the display cutoff', async ({ page }) => {
  await page.goto('/#' + encodeURIComponent(JSON.stringify({ cols: [['Ry(π/524288)']] })))
  await waitForStartupReady(page, { waitForStateVector: true })
  const pixels = await secondCirclePixels(page)
  expect(pixels.needle[0]).toBeLessThan(100)
})

test('state vector probability disk draws the blue inset border', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const center = defaultFilledStateCircleCenter(box.width, box.height)
  const samples = await sampleCanvasPixels(page, canvas, [
    { name: 'rimA', x: center.x + 30.0, y: center.y },
    { name: 'rimB', x: center.x + 30.5, y: center.y },
    { name: 'rimC', x: center.x + 31.0, y: center.y },
  ])
  const bestRimDistance = Math.min(
    pixelRgbDistance(samples.rimA, FLEXOKI_BLUE_400),
    pixelRgbDistance(samples.rimB, FLEXOKI_BLUE_400),
    pixelRgbDistance(samples.rimC, FLEXOKI_BLUE_400),
  )

  expect(bestRimDistance).toBeLessThanOrEqual(90)
})
