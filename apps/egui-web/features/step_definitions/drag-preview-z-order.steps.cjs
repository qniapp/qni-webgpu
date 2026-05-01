const assert = require('node:assert/strict')
const { Then, When } = require('@cucumber/cucumber')

const { assertDragPreviewAboveOverlay } = require('../support/assertions.cjs')
const {
  dragPointer,
  getDragPreviewAboveStatePanelProbe,
  sampleCanvasPixels,
} = require('../support/egui-helpers.cjs')

When('I drag the palette gate from the palette over the state panel', async function () {
  assert.ok(this.page, 'expected egui page to be open')

  const canvas = this.page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })
  const box = await canvas.boundingBox()
  assert.ok(box, 'expected egui canvas to be measurable')

  const probe = getDragPreviewAboveStatePanelProbe(box.width, box.height)
  const beforeDrag = await sampleCanvasPixels(this.page, canvas, [
    probe.dragFillPoint,
    probe.sourceFillPoint,
  ])

  await dragPointer(this.page, probe.source, probe.handleCenter, 8, false)
  await this.page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(this.page, canvas, [probe.dragFillPoint])
  this.dragPreviewZOrder = {
    before: beforeDrag.fill,
    during: duringDrag.fill,
    source: beforeDrag.sourceFill,
  }
})

Then('the dragged gate stays above the state panel overlay', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  assert.ok(this.dragPreviewZOrder, 'expected drag preview samples to be captured')

  try {
    assertDragPreviewAboveOverlay(this.dragPreviewZOrder)
  } finally {
    await this.page.mouse.up().catch(() => {})
  }
})
