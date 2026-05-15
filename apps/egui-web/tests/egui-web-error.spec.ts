import { expect, test } from '@playwright/test'
import {
  assertDragPreviewAboveOverlay,
  chromium,
  dragPointer,
  getDragPreviewAboveStatePanelProbe,
  getPaletteGateCenter,
  getPlainChromiumLaunchOptions,
  getWebServerConfig,
  isDragPreviewFill,
  isGateBodyFill,
  isRegularGateFill,
  pixelRgbDistance,
  readEguiError,
  readMeasurementOutcomes,
  readStateVector,
  releasePointer,
  sampleCanvasPixels,
  waitForAppReady,
  waitForBlochVectorsApprox,
  waitForCanvasContent,
  waitForStartupReady,
  waitForStateVectorApprox,
  waitForStateVectorLength,
  type CanvasPixel,
  type CircularBodySignature,
  type PixelSamplePoint,
  type Point,
} from './support/egui-web-spec-helpers'

test('default chromium shows a visible WebGPU error instead of a blank page', async () => {
  const plainChromium = getPlainChromiumLaunchOptions({
    env: process.env,
    defaultPath: chromium.executablePath(),
  })
  const { url } = getWebServerConfig()

  const browser = await chromium.launch(plainChromium)

  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } })
    await page.goto(new URL('/', url).toString(), { waitUntil: 'load' })
    await waitForAppReady(page)

    let error: string | null = null
    for (let attempt = 0; attempt < 200; attempt += 1) {
      error = await readEguiError(page)
      if (error) break
      await page.waitForTimeout(100)
    }
    const errorLocator = page.locator('[data-testid="webgpu-error"]')
    await errorLocator.waitFor({ state: 'visible' })
    expect({ errorPresent: error !== null, visibleTextMentionsWebGpu: (await errorLocator.innerText()).includes('WebGPU') }).toEqual({
      errorPresent: true,
      visibleTextMentionsWebGpu: true,
    })
  } finally {
    await browser.close()
  }
})
