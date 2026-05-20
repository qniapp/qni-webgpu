const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const {
  buildDragOperation,
  buildScreenshotPlan,
  getGateIndex,
  parseWire,
  parseOperations,
} = require('../test-support/agent-visual-command.ts')

const rootDir = path.join(__dirname, '..')
const repoRoot = path.join(rootDir, '..', '..')
const readText = (filePath: string) => fs.readFile(filePath, 'utf8')

test('agent visual command resolves palette gate aliases', () => {
  const aliases = ['H', 'X', 'sqrtx', 's†', 'tdagger', 'swap', 'control', 'anti-control', '◦', 'bloch', 'sphere', '|0>', '|0⟩', 'write0', '|1>', 'write1', 'measure', 'm', 'chance', 'spacer', '…', 'qft', 'qft†', 'amps']
  assert.deepEqual(aliases.map(getGateIndex), [0, 1, 4, 6, 8, 13, 14, 15, 15, 16, 16, 17, 17, 17, 18, 18, 19, 19, 20, 21, 21, 22, 23, 24])
})

test('agent visual command parses q-prefixed and numeric wires', () => {
  assert.deepEqual(['q0', '1'].map(parseWire), [0, 1])
})

test('agent visual command builds drag coordinates from semantic gate placement', () => {
  const operation = buildDragOperation({
    cssWidth: 1000,
    gate: 'x',
    wire: 'q1',
    slot: 2,
  })

  assert.deepEqual(operation, {
    gate: 'x',
    gateIndex: 1,
    wire: 1,
    slot: 2,
    from: { x: 208, y: 100 },
    to: { x: 274, y: 312 },
  })
})

test('agent visual command builds row-two coordinates after display gates move out', () => {
  const operation = buildDragOperation({
    cssWidth: 1000,
    gate: 'qft',
    wire: 'q0',
    slot: 0,
  })

  assert.deepEqual({ gateIndex: operation.gateIndex, from: operation.from }, { gateIndex: 22, from: { x: 448, y: 148 } })
})

test('agent visual command builds Display section coordinates', () => {
  const operation = buildDragOperation({
    cssWidth: 1000,
    gate: 'chance',
    wire: 'q0',
    slot: 0,
  })

  assert.deepEqual({ gateIndex: operation.gateIndex, from: operation.from }, { gateIndex: 20, from: { x: 841, y: 100 } })
})

test('agent visual command supports egui content vertical offset', () => {
  const operation = buildDragOperation({
    cssWidth: 1000,
    gate: 'h',
    wire: 'q0',
    slot: 0,
    verticalOffset: 8,
  })

  assert.deepEqual({ fromY: operation.from.y, toY: operation.to.y }, { fromY: 108, toY: 264 })
})

test('agent visual command parses comma separated operations', () => {
  assert.deepEqual(parseOperations('H:q0:0,C:q0:1,X:q1:1'), [
    { gate: 'H', wire: 'q0', slot: 0 },
    { gate: 'C', wire: 'q0', slot: 1 },
    { gate: 'X', wire: 'q1', slot: 1 },
  ])
})

test('agent visual command writes page screenshots by default', () => {
  assert.deepEqual({
    defaultPlan: buildScreenshotPlan({ command: 'drag' }),
    explicitPlan: buildScreenshotPlan({ command: 'drag', out: 'page.png', canvasOut: 'canvas.png' }),
  }, {
    defaultPlan: { pageOut: 'output/playwright/agent-visual/drag.png', canvasOut: null },
    explicitPlan: { pageOut: 'page.png', canvasOut: 'canvas.png' },
  })
})

test('agent visual CLI uses a generic name without a compatibility wrapper', async () => {
  const accessOk = async (filePath: string): Promise<boolean> => fs.access(filePath).then(() => true, () => false)
  const docs = await readText(path.join(repoRoot, 'docs', 'web.md'))
  const agents = await readText(path.join(repoRoot, 'AGENTS.md'))
  assert.deepEqual({
    tsEntrypoint: await accessOk(path.join(rootDir, 'scripts', 'agent-visual.ts')),
    legacyEntrypoint: await accessOk(path.join(rootDir, 'scripts', 'agent-visual.cjs')),
    codexEntrypoint: await accessOk(path.join(rootDir, 'scripts', 'codex-visual.cjs')),
    docsUseTs: /scripts\/agent-visual\.ts/.test(docs),
    docsAvoidCjs: !/scripts\/agent-visual\.cjs/.test(docs),
    agentsMentionCompatibility: /後方互換/.test(agents),
    agentsMentionNoWrappers: /残さない/.test(agents),
  }, {
    tsEntrypoint: true,
    legacyEntrypoint: false,
    codexEntrypoint: false,
    docsUseTs: true,
    docsAvoidCjs: true,
    agentsMentionCompatibility: true,
    agentsMentionNoWrappers: true,
  })
})

export {}
