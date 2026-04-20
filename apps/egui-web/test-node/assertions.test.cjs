const test = require('node:test')
const assert = require('node:assert/strict')

const { assertDragPreviewAboveOverlay } = require('../features/support/assertions.cjs')

test('assertDragPreviewAboveOverlay accepts a drag preview that matches the source gate fill while differing from the hidden baseline', () => {
  assert.doesNotThrow(() => {
    assertDragPreviewAboveOverlay({
      before: [255, 255, 255, 255],
      during: [51, 158, 140, 255],
      source: [48, 160, 139, 255],
    })
  })
})

test('assertDragPreviewAboveOverlay rejects a drag preview that still matches the hidden baseline', () => {
  assert.throws(
    () => {
      assertDragPreviewAboveOverlay({
        before: [255, 255, 255, 255],
        during: [248, 249, 250, 255],
        source: [51, 158, 140, 255],
      })
    },
    /hidden baseline/
  )
})

test('assertDragPreviewAboveOverlay rejects a drag preview that drifts away from the sampled source gate fill', () => {
  assert.throws(
    () => {
      assertDragPreviewAboveOverlay({
        before: [255, 255, 255, 255],
        during: [200, 80, 90, 255],
        source: [51, 158, 140, 255],
      })
    },
    /source gate fill/
  )
})
