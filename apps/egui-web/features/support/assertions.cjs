const assert = require('node:assert/strict')

const rgbDistance = (left, right) =>
  [0, 1, 2].reduce((total, channel) => total + Math.abs((left?.[channel] ?? 0) - (right?.[channel] ?? 0)), 0)

const assertDragPreviewAboveOverlay = ({
  before,
  during,
  source,
  hiddenBaselineMinDiff = 120,
  sourceTolerance = 40,
}) => {
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

module.exports = {
  rgbDistance,
  assertDragPreviewAboveOverlay,
}
