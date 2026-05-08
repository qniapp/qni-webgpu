import assert = require('node:assert/strict')
import type { CanvasPixel, DragPreviewZOrderSamples } from './support-types'

type DragPreviewZOrderAssertionOptions = DragPreviewZOrderSamples & {
  hiddenBaselineMinDiff?: number
  sourceMinDiff?: number
  dragFillTolerance?: number
}

const QNI_INTERMEDIATE_FILL: CanvasPixel = [168, 85, 247, 255]

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

  const dragFillDiff = rgbDistance(QNI_INTERMEDIATE_FILL, during)
  assert.ok(
    dragFillDiff <= dragFillTolerance,
    `expected dragged palette gate fill to use qni intermediate purple ` +
      `(diff=${dragFillDiff}, tolerance=${dragFillTolerance}, expected=${QNI_INTERMEDIATE_FILL}, during=${during})`
  )
}
