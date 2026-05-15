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

export const dragPreviewAboveOverlayIssue = ({
  before,
  during,
  source,
  hiddenBaselineMinDiff = 120,
  sourceMinDiff = 120,
  dragFillTolerance = 80,
}: DragPreviewZOrderAssertionOptions): string | null => {
  const hiddenBaselineDiff = rgbDistance(before, during)
  if (hiddenBaselineDiff <= hiddenBaselineMinDiff) {
    return `hidden baseline diff too small (diff=${hiddenBaselineDiff}, before=${before}, during=${during})`
  }

  const sourceDiff = rgbDistance(source, during)
  if (sourceDiff <= sourceMinDiff) {
    return `green source fill diff too small (diff=${sourceDiff}, min=${sourceMinDiff}, source=${source}, during=${during})`
  }

  const dragFillDiff = rgbDistance(DRAG_PREVIEW_FILL, during)
  if (dragFillDiff > dragFillTolerance) {
    return `not Flexoki purple-600 (diff=${dragFillDiff}, tolerance=${dragFillTolerance}, expected=${DRAG_PREVIEW_FILL}, during=${during})`
  }

  return null
}
