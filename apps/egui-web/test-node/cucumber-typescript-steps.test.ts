const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const stepDefinitionsDir = path.join(rootDir, 'features', 'step_definitions')
const supportDir = path.join(rootDir, 'features', 'support')
const testSupportDir = path.join(rootDir, 'test-support')

const readStep = (name: string) => fs.readFile(path.join(stepDefinitionsDir, name), 'utf8')
const readSupport = (name: string) => fs.readFile(path.join(supportDir, name), 'utf8')
const accessOk = (filePath: string): Promise<boolean> => fs.access(filePath).then(() => true, () => false)

test('typescript cucumber steps share support module type contracts', async () => {
  const supportTypes = await readSupport('support-types.ts')
  const stepFiles = ['startup-success.steps.ts', 'plain-chromium-error.steps.ts', 'drag-preview-z-order.steps.ts']
  const stepSources = await Promise.all(stepFiles.map(readStep))

  assert.deepEqual({
    supportExports: [
      /export type EguiWorld =/.test(supportTypes),
      /export type BrowserSupport =/.test(supportTypes),
      /export type EguiHelpers =/.test(supportTypes),
      /export type AssertionsSupport =/.test(supportTypes),
      /export type WindowWithEguiError =/.test(supportTypes),
      /dragPreviewZOrder\?: DragPreviewZOrderSamples/.test(supportTypes),
    ],
    hasLegacyWorldTypes: await accessOk(path.join(supportDir, 'world-types.ts')),
    stepImportsSharedTypes: stepSources.map((source) => /import type \{[^}]*EguiWorld[^}]*\} from '..\/support\/support-types'/.test(source)),
    stepUsesCjsBrowser: stepSources.map((source) => /browser\.cjs/.test(source)),
    stepDeclaresLocalTypes: stepSources.map((source) => /type (AssertionsSupport|BrowserSupport|DragPreviewProbe|EguiHelpers|EguiWorld|PixelSamplePoint|Point|WindowWithEguiError)\b/.test(source)),
  }, {
    supportExports: [true, true, true, true, true, true],
    hasLegacyWorldTypes: false,
    stepImportsSharedTypes: [true, true, true],
    stepUsesCjsBrowser: [false, false, false],
    stepDeclaresLocalTypes: [false, false, false],
  })
})

test('drag preview z-order cucumber steps are implemented in TypeScript', async () => {
  const source = await readStep('drag-preview-z-order.steps.ts')

  assert.deepEqual({
    usesDragPreviewState: /dragPreviewZOrder/.test(source),
    usesSharedAssertion: /assertDragPreviewAboveOverlay/.test(source),
    avoidsCjsAssertion: !/assertions\.cjs/.test(source),
  }, { usesDragPreviewState: true, usesSharedAssertion: true, avoidsCjsAssertion: true })
})

test('drag preview assertions support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'assertions.ts')),
    cjs: await accessOk(path.join(supportDir, 'assertions.cjs')),
  }, { ts: true, cjs: false })
})

test('browser support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'browser.ts')),
    cjs: await accessOk(path.join(supportDir, 'browser.cjs')),
  }, { ts: true, cjs: false })
})

test('server support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'server.ts')),
    cjs: await accessOk(path.join(supportDir, 'server.cjs')),
  }, { ts: true, cjs: false })
})

test('world support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'world.ts')),
    cjs: await accessOk(path.join(supportDir, 'world.cjs')),
  }, { ts: true, cjs: false })
})

test('hooks support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'hooks.ts')),
    cjs: await accessOk(path.join(supportDir, 'hooks.cjs')),
  }, { ts: true, cjs: false })
})

test('bootstrap support entrypoint is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'bootstrap.ts')),
    cjs: await accessOk(path.join(supportDir, 'bootstrap.cjs')),
  }, { ts: true, cjs: false })
})

test('egui helpers support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(supportDir, 'egui-helpers.ts')),
    cjs: await accessOk(path.join(supportDir, 'egui-helpers.cjs')),
  }, { ts: true, cjs: false })
})

test('browser launch test support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(testSupportDir, 'browser-launch.ts')),
    cjs: await accessOk(path.join(testSupportDir, 'browser-launch.cjs')),
  }, { ts: true, cjs: false })
})

test('web server test support is implemented in TypeScript', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(testSupportDir, 'web-server.ts')),
    cjs: await accessOk(path.join(testSupportDir, 'web-server.cjs')),
  }, { ts: true, cjs: false })
})

test('agent visual command test support is implemented in TypeScript with a generic name', async () => {
  assert.deepEqual({
    ts: await accessOk(path.join(testSupportDir, 'agent-visual-command.ts')),
    cjs: await accessOk(path.join(testSupportDir, 'codex-visual-command.cjs')),
  }, { ts: true, cjs: false })
})

export {}
