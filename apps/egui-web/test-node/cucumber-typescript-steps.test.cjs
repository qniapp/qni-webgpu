const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const stepDefinitionsDir = path.join(rootDir, 'features', 'step_definitions')
const supportDir = path.join(rootDir, 'features', 'support')

const readStep = (name) => fs.readFile(path.join(stepDefinitionsDir, name), 'utf8')
const readSupport = (name) => fs.readFile(path.join(supportDir, name), 'utf8')

test('typescript cucumber steps share support module type contracts', async () => {
  const supportTypes = await readSupport('support-types.ts')

  assert.match(supportTypes, /export type EguiWorld =/)
  assert.match(supportTypes, /export type BrowserSupport =/)
  assert.match(supportTypes, /export type EguiHelpers =/)
  assert.match(supportTypes, /export type AssertionsSupport =/)
  assert.match(supportTypes, /export type WindowWithEguiError =/)
  assert.match(supportTypes, /dragPreviewZOrder\?: DragPreviewZOrderSamples/)
  await assert.rejects(() => fs.access(path.join(supportDir, 'world-types.ts')), /ENOENT/)

  for (const stepFile of [
    'startup-success.steps.ts',
    'plain-chromium-error.steps.ts',
    'drag-preview-z-order.steps.ts',
  ]) {
    const source = await readStep(stepFile)

    assert.match(source, /import type \{[^}]*EguiWorld[^}]*\} from '..\/support\/support-types'/)
    assert.doesNotMatch(source, /browser\.cjs/)
    assert.doesNotMatch(
      source,
      /type (AssertionsSupport|BrowserSupport|DragPreviewProbe|EguiHelpers|EguiWorld|PixelSamplePoint|Point|WindowWithEguiError)\b/
    )
  }
})

test('drag preview z-order cucumber steps are implemented in TypeScript', async () => {
  const source = await readStep('drag-preview-z-order.steps.ts')

  assert.match(source, /dragPreviewZOrder/)
  assert.match(source, /assertDragPreviewAboveOverlay/)
  assert.doesNotMatch(source, /assertions\.cjs/)
})

test('drag preview assertions support is implemented in TypeScript', async () => {
  await assert.doesNotReject(() => fs.access(path.join(supportDir, 'assertions.ts')))
  await assert.rejects(() => fs.access(path.join(supportDir, 'assertions.cjs')), /ENOENT/)
})

test('browser support is implemented in TypeScript', async () => {
  await assert.doesNotReject(() => fs.access(path.join(supportDir, 'browser.ts')))
  await assert.rejects(() => fs.access(path.join(supportDir, 'browser.cjs')), /ENOENT/)
})
