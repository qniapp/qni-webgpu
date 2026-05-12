import assert = require('node:assert/strict')
import type { CanvasPixel, DragPreviewZOrderSamples } from './support-types'

type DragPreviewZOrderAssertionOptions = DragPreviewZOrderSamples & {
  hiddenBaselineMinDiff?: number
  sourceMinDiff?: number
  dragFillTolerance?: number
}

// Flexoki purple-600 — drag preview / semantic-intermediate fill.
const DRAG_PREVIEW_FILL: CanvasPixel = [94, 64, 157, 255]

export const rgbDistance = (left: CanvasPixel | undefined, right: CanvasPixel | undefined): number =>
  [0, 1, 2].reduce((total, channel) => total + Math.abs((left?.[channel] ?? 0) - (right?.[channel] ?? 0)), 0)

export const assertDragPreviewAboveOverlay = ({
  before,
  during,
  source,
  hiddenBaselineMinDiff = 120,
  sourceMinDiff = 120,
  dragFillTolerance = 80,
}: DragPreviewZOrderAssertionOptions): void => {
  const hiddenBaselineDiff = rgbDistance(before, during)
  assert.ok(
    hiddenBaselineDiff > hiddenBaselineMinDiff,
    `expected drag preview fill to differ from the hidden baseline ` +
      `(diff=${hiddenBaselineDiff}, before=${before}, during=${during})`
  )

  const sourceDiff = rgbDistance(source, during)
  assert.ok(
    sourceDiff > sourceMinDiff,
    `expected dragged palette gate fill to switch away from the green source fill ` +
      `(diff=${sourceDiff}, min=${sourceMinDiff}, source=${source}, during=${during})`
  )

  const dragFillDiff = rgbDistance(DRAG_PREVIEW_FILL, during)
  assert.ok(
    dragFillDiff <= dragFillTolerance,
    `expected dragged palette gate fill to use Flexoki purple-600 ` +
      `(diff=${dragFillDiff}, tolerance=${dragFillTolerance}, expected=${DRAG_PREVIEW_FILL}, during=${during})`
  )
}
