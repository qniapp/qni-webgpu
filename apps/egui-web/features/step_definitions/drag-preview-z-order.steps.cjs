const assert = require('node:assert/strict')
const { Then, When } = require('@cucumber/cucumber')

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
  const beforeDrag = await sampleCanvasPixels(this.page, canvas, [probe.dragFillPoint])

  await dragPointer(this.page, probe.source, probe.handleCenter, 8, false)
  await this.page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(this.page, canvas, [probe.dragFillPoint])
  this.dragPreviewZOrder = {
    before: beforeDrag.fill,
    during: duringDrag.fill,
  }
})

Then('the dragged gate stays above the state panel overlay', async function () {
  assert.ok(this.page, 'expected egui page to be open')
  assert.ok(this.dragPreviewZOrder, 'expected drag preview samples to be captured')

  try {
    const { before, during } = this.dragPreviewZOrder
    const diff =
      Math.abs(before[0] - during[0]) +
      Math.abs(before[1] - during[1]) +
      Math.abs(before[2] - during[2])

    assert.ok(
      diff > 120,
      `expected drag preview fill to differ from the hidden baseline (diff=${diff}, before=${before}, during=${during})`
    )
    assert.ok(
      during[1] > during[0] + 40,
      `expected dragged palette gate fill to stay green-dominant above the overlay (during=${during})`
    )
  } finally {
    await this.page.mouse.up().catch(() => {})
  }
})
