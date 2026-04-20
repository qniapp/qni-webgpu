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
