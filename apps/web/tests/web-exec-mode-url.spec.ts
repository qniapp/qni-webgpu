import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  sampleCanvasPixels,
  waitForStartupReady,
  type CanvasPixel,
  type PixelSamplePoint,
} from './support/web-spec-helpers'

const EXEC_MODE_GPU_FILL: CanvasPixel = [32, 94, 166, 255] // Flexoki blue-600 #205EA6
const circuitHash = (cols: unknown[]): string => encodeURIComponent(JSON.stringify({ cols }))

const execModeProbePoints = (cssWidth: number): PixelSamplePoint[] => [
  { name: 'local', x: cssWidth - 100, y: 23 },
  { name: 'gpu', x: cssWidth - 30, y: 23 },
]

const waitForGpuModeFill = async (page: Page): Promise<void> => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const points = execModeProbePoints(box.width)
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const pixels = await sampleCanvasPixels(page, canvas, points)
    if (pixelRgbDistance(pixels.gpu, EXEC_MODE_GPU_FILL) < 36) return
    await page.waitForTimeout(50)
  }
  throw new Error('GPU mode did not reach expected fill')
}

test('GPU mode toggle writes the mode URL option', async ({ page }) => {
  await page.goto('/')
  await waitForStartupReady(page, { waitForStateVector: true })

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const points = execModeProbePoints(box.width)
  await page.mouse.click(box.x + points[1].x, box.y + points[1].y)

  await expect.poll(() => new URL(page.url()).searchParams.get('mode')).toBe('gpu')
})

test('GPU mode URL option restores the toggle on reload', async ({ page }) => {
  await page.goto(`/?mode=gpu#${circuitHash([])}`)
  await waitForStartupReady(page)

  await waitForGpuModeFill(page)
  const mode = new URL(page.url()).searchParams.get('mode')

  expect(mode).toBe('gpu')
})

test('Local mode toggle removes mode without changing the circuit hash', async ({ page }) => {
  const hash = circuitHash([['H']])
  await page.goto(`/?foo=1&mode=gpu#${hash}`)
  await waitForStartupReady(page)
  await waitForGpuModeFill(page)

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const points = execModeProbePoints(box.width)
  const initialHash = new URL(page.url()).hash
  await page.mouse.click(box.x + points[0].x, box.y + points[0].y)

  await expect.poll(() => {
    const url = new URL(page.url())
    return `${url.searchParams.get('mode') ?? ''}|${url.searchParams.get('foo') ?? ''}|${url.hash}`
  }).toBe(`|1|${initialHash}`)
})
