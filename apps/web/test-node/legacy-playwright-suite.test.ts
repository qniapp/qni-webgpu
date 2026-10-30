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

test('legacy Playwright suite imports and uses shared drag and canvas-content helpers', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')
  const sharedImports = source.match(/import\s*\{([^}]+)\}\s*from\s*['"]\.\/support\/web-spec-helpers['"]/)?.[1] ?? ''

  assert.deepEqual({
    hasInlineHelpers: /\b(?:const|let|var|function)\s+(?:screenshotWithRetry|sampleCanvasPixels|dragPointer|waitForCanvasContent)\b/.test(source),
    importsSharedHelpers: ['dragPointer', 'waitForCanvasContent'].every(name => new RegExp(`\\b${name}\\b`).test(sharedImports)),
    usesSharedDragHelper: /\bawait\s+dragPointer\(page,/.test(source),
    usesSharedCanvasHelper: /\bawait\s+waitForCanvasContent\(page,/.test(source),
  }, { hasInlineHelpers: false, importsSharedHelpers: true, usesSharedDragHelper: true, usesSharedCanvasHelper: true })
})

test('legacy Playwright suite uses the configured page without inline launch args or server URLs', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.deepEqual({
    usesRelativeNavigation: /\bpage\.goto\(['"]\/['"]\)/.test(source),
    hasInlineGpuDisablingArgs: /--disable-(?:gpu|software-rasterizer)\b/.test(source),
    hasHardcodedServerUrl: /https?:\/\/(?:127\.0\.0\.1|localhost)(?::\d+)?\b/.test(source),
  }, { usesRelativeNavigation: true, hasInlineGpuDisablingArgs: false, hasHardcodedServerUrl: false })
})

test('legacy Playwright suite avoids shared fixed screenshot paths that would collide under parallel workers', async () => {
  const source = await fs.readFile(legacySuitePath, 'utf8')

  assert.deepEqual({
    hasSharedTmpScreenshotPath: /path:\s*['"]\/tmp\/qni-webgpu-[^'"]+['"]/.test(source),
    usesTestInfoOutputPath: /testInfo\.outputPath\(/.test(source),
  }, { hasSharedTmpScreenshotPath: false, usesTestInfoOutputPath: true })
})

export {}
