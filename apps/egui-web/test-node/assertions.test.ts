const test = require('node:test')
const assert = require('node:assert/strict')

const { assertDragPreviewAboveOverlay } = require('../features/support/assertions.ts')

test('assertDragPreviewAboveOverlay accepts qni intermediate purple while differing from the hidden baseline and source', () => {
  assert.doesNotThrow(() => {
    assertDragPreviewAboveOverlay({
      before: [255, 255, 255, 255],
      during: [168, 85, 247, 255],
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

test('assertDragPreviewAboveOverlay rejects a drag preview that stays green like the sampled source gate fill', () => {
  assert.throws(
    () => {
      assertDragPreviewAboveOverlay({
        before: [255, 255, 255, 255],
        during: [51, 158, 140, 255],
        source: [51, 158, 140, 255],
      })
    },
    /green source fill/
  )
})

test('assertDragPreviewAboveOverlay rejects a drag preview that is not qni intermediate purple', () => {
  assert.throws(
    () => {
      assertDragPreviewAboveOverlay({
        before: [255, 255, 255, 255],
        during: [200, 80, 90, 255],
        source: [51, 158, 140, 255],
      })
    },
    /qni intermediate purple/
  )
})

export {}
