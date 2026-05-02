const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const stepDefinitionsDir = path.join(rootDir, 'features', 'step_definitions')
const supportDir = path.join(rootDir, 'features', 'support')

const readStep = (name) => fs.readFile(path.join(stepDefinitionsDir, name), 'utf8')
const readSupport = (name) => fs.readFile(path.join(supportDir, name), 'utf8')

test('typescript cucumber steps share the typed egui world state', async () => {
  const worldTypes = await readSupport('world-types.ts')

  assert.match(worldTypes, /export type EguiWorld =/)
  assert.match(worldTypes, /export type DragPreviewZOrderSamples =/)
  assert.match(worldTypes, /dragPreviewZOrder\?: DragPreviewZOrderSamples/)

  for (const stepFile of [
    'startup-success.steps.ts',
    'plain-chromium-error.steps.ts',
    'drag-preview-z-order.steps.ts',
  ]) {
    const source = await readStep(stepFile)

    assert.match(source, /import type \{[^}]*EguiWorld[^}]*\} from '..\/support\/world-types'/)
    assert.doesNotMatch(source, /type EguiWorld =/)
  }
})

test('drag preview z-order cucumber steps are implemented in TypeScript', async () => {
  const source = await readStep('drag-preview-z-order.steps.ts')

  assert.match(source, /dragPreviewZOrder/)
  assert.match(source, /assertDragPreviewAboveOverlay/)
})
