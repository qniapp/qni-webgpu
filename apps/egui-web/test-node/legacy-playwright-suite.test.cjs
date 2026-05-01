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

test('legacy Playwright plain-chromium lane uses shared launch/server policy modules', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.match(source, /getPlainChromiumLaunchOptions/)
  assert.match(source, /getWebServerConfig/)
  assert.doesNotMatch(source, /args:\s*\['--disable-gpu',\s*'--disable-software-rasterizer'\]/)
  assert.doesNotMatch(source, /http:\/\/127\.0\.0\.1:4174\//)
})

test('legacy Playwright suite avoids shared fixed screenshot paths that would collide under parallel workers', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.doesNotMatch(source, /path:\s*['"]\/tmp\/qni-egui-webgpu-[^'"]+['"]/) 
  assert.match(source, /testInfo\.outputPath\(/)
})

test('legacy Playwright canvas smoke does not duplicate Bell-state circuit coverage', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')
  const canvasSmoke = source.match(/test\('egui webgpu canvas renders content'[\s\S]*?\n}\)/)?.[0] || ''

  assert.doesNotMatch(canvasSmoke, /expectedBell/)
  assert.doesNotMatch(canvasSmoke, /controlX/)
})

test('legacy Playwright drag-growth test uses the smallest circuit that covers delayed growth', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')
  const dragGrowthTest = source.match(/test\('dragging does not grow state vector until drop'[\s\S]*?\n}\)/)?.[0] || ''

  assert.doesNotMatch(dragGrowthTest, /targetY1/)
  assert.match(dragGrowthTest, /waitForStateVectorLength\(page, 4\)/)
  assert.match(dragGrowthTest, /waitForStateVectorLength\(page, 16\)/)
})
