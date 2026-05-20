const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const legacySuitePath = path.join(rootDir, 'tests', 'web.spec.ts')

test('legacy Playwright suite is TypeScript without a compatibility wrapper', async () => {
  const accessOk = (filePath: string): Promise<boolean> => fs.access(filePath).then(() => true, () => false)
  assert.deepEqual({
    tsSuite: await accessOk(legacySuitePath),
    jsWrapper: await accessOk(path.join(rootDir, 'tests', 'web.spec.js')),
  }, { tsSuite: true, jsWrapper: false })
})

test('legacy Playwright suite reuses the shared drag and pixel helpers for the drag-preview lane', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.deepEqual({
    inlineScreenshotHelper: /\bconst screenshotWithRetry = async\b/.test(source),
    inlinePixelHelper: /\bconst sampleCanvasPixels = async\b/.test(source),
    inlineDragHelper: /\bconst dragPointer = async\b/.test(source),
    importsSharedHelpers: /dragPointer,[\s\S]*sampleCanvasPixels,/.test(source),
  }, { inlineScreenshotHelper: false, inlinePixelHelper: false, inlineDragHelper: false, importsSharedHelpers: true })
})

test('legacy Playwright plain-chromium lane uses shared launch/server policy modules', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.deepEqual({
    usesSharedBrowserPolicy: /getPlainChromiumLaunchOptions/.test(source),
    usesSharedServerPolicy: /getWebServerConfig/.test(source),
    hasInlineGpuDisablingArgs: /args:\s*\['--disable-gpu',\s*'--disable-software-rasterizer'\]/.test(source),
    hasHardcodedServerUrl: /http:\/\/127\.0\.0\.1:4174\//.test(source),
  }, { usesSharedBrowserPolicy: true, usesSharedServerPolicy: true, hasInlineGpuDisablingArgs: false, hasHardcodedServerUrl: false })
})

test('legacy Playwright suite avoids shared fixed screenshot paths that would collide under parallel workers', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.deepEqual({
    hasSharedTmpScreenshotPath: /path:\s*['"]\/tmp\/qni-webgpu-[^'"]+['"]/.test(source),
    usesTestInfoOutputPath: /testInfo\.outputPath\(/.test(source),
  }, { hasSharedTmpScreenshotPath: false, usesTestInfoOutputPath: true })
})

export {}
