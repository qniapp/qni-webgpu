import { expect, test } from '@playwright/test'
import {
  pixelRgbDistance,
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
