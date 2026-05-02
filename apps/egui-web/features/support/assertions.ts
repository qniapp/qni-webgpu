import assert = require('node:assert/strict')
import type { CanvasPixel, DragPreviewZOrderSamples } from './support-types'

type DragPreviewZOrderAssertionOptions = DragPreviewZOrderSamples & {
  hiddenBaselineMinDiff?: number
  sourceTolerance?: number
}

export const rgbDistance = (left: CanvasPixel | undefined, right: CanvasPixel | undefined): number =>
  [0, 1, 2].reduce((total, channel) => total + Math.abs((left?.[channel] ?? 0) - (right?.[channel] ?? 0)), 0)

export const assertDragPreviewAboveOverlay = ({
  before,
  during,
  source,
  hiddenBaselineMinDiff = 120,
  sourceTolerance = 40,
}: DragPreviewZOrderAssertionOptions): void => {
  const hiddenBaselineDiff = rgbDistance(before, during)
  assert.ok(
    hiddenBaselineDiff > hiddenBaselineMinDiff,
    `expected drag preview fill to differ from the hidden baseline ` +
      `(diff=${hiddenBaselineDiff}, before=${before}, during=${during})`
  )

  const sourceDiff = rgbDistance(source, during)
  assert.ok(
    sourceDiff <= sourceTolerance,
    `expected dragged palette gate fill to stay aligned with the sampled source gate fill ` +
      `(diff=${sourceDiff}, tolerance=${sourceTolerance}, source=${source}, during=${during})`
  )
}
