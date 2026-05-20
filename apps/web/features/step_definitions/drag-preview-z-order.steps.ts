import { Then, When } from '@cucumber/cucumber'
import assert = require('node:assert/strict')
import type { Page } from 'playwright'
import type { AssertionsSupport, EguiHelpers, EguiWorld } from '../support/support-types'

const { dragPreviewAboveOverlayIssue } = require('../support/assertions.ts') as AssertionsSupport
const {
  dragPointer,
  getDragPreviewAboveStatePanelProbe,
  sampleCanvasPixels,
} = require('../support/egui-helpers.ts') as EguiHelpers

const requirePage = (world: EguiWorld): Page => {
  if (!world.page) {
    throw new Error('expected web page to be open')
  }
  return world.page
}

When('I drag the palette gate from the palette over the state panel', async function (this: EguiWorld) {
  const page = requirePage(this)
  const canvas = page.locator('#egui-canvas')
  await canvas.waitFor({ state: 'visible' })
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('expected egui canvas to be measurable')
  }

  const probe = getDragPreviewAboveStatePanelProbe(box.width, box.height)
  const beforeDrag = await sampleCanvasPixels(page, canvas, [
    probe.dragFillPoint,
    probe.sourceFillPoint,
  ])

  await dragPointer(page, probe.source, probe.handleCenter, 8, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [probe.dragFillPoint])
  this.dragPreviewZOrder = {
    before: beforeDrag.fill,
    during: duringDrag.fill,
    source: beforeDrag.sourceFill,
  }
})

Then('the dragged gate stays above the state panel overlay', async function (this: EguiWorld) {
  const page = requirePage(this)
  if (!this.dragPreviewZOrder) {
    throw new Error('expected drag preview samples to be captured')
  }

  try {
    assert.equal(dragPreviewAboveOverlayIssue(this.dragPreviewZOrder), null)
  } finally {
    await page.mouse.up().catch(() => {})
  }
})
