const test = require('node:test')
const assert = require('node:assert/strict')

const { dragPreviewAboveOverlayIssue } = require('../features/support/assertions.ts')

test('dragPreviewAboveOverlayIssue accepts Flexoki purple-600 while differing from the hidden baseline and source', () => {
  assert.equal(dragPreviewAboveOverlayIssue({
    before: [255, 255, 255, 255],
    during: [94, 64, 157, 255],
    source: [48, 160, 139, 255],
  }), null)
})

test('dragPreviewAboveOverlayIssue rejects a drag preview that still matches the hidden baseline', () => {
  assert.match(dragPreviewAboveOverlayIssue({
    before: [255, 255, 255, 255],
    during: [248, 249, 250, 255],
    source: [51, 158, 140, 255],
  }) ?? '', /hidden baseline/)
})

test('dragPreviewAboveOverlayIssue rejects a drag preview that stays green like the sampled source gate fill', () => {
  assert.match(dragPreviewAboveOverlayIssue({
    before: [255, 255, 255, 255],
    during: [51, 158, 140, 255],
    source: [51, 158, 140, 255],
  }) ?? '', /green source fill/)
})

test('dragPreviewAboveOverlayIssue rejects a drag preview that is not Flexoki purple-600', () => {
  assert.match(dragPreviewAboveOverlayIssue({
    before: [255, 255, 255, 255],
    during: [200, 80, 90, 255],
    source: [51, 158, 140, 255],
  }) ?? '', /Flexoki purple-600/)
})

export {}
