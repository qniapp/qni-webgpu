const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const legacySuitePath = path.join(rootDir, 'tests', 'egui-web.spec.js')

test('legacy Playwright suite reuses the shared drag and pixel helpers for the drag-preview lane', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.doesNotMatch(source, /\bconst screenshotWithRetry = async\b/)
  assert.doesNotMatch(source, /\bconst sampleCanvasPixels = async\b/)
  assert.doesNotMatch(source, /\bconst dragPointer = async\b/)
  assert.match(source, /dragPointer,[\s\S]*sampleCanvasPixels,/)
})
