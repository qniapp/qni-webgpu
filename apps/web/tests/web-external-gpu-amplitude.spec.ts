import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  readAmplitudeCell,
  sampleCanvasPixels,
  type CanvasPixel,
  type PixelSamplePoint,
  type Point,
  waitForStartupReady,
} from './support/web-spec-helpers'

const EXEC_MODE_GPU_FILL: CanvasPixel = [32, 94, 166, 255] // Flexoki blue-600 #205EA6
const RUN_GPU_BUTTON_POINT: Point = { x: 334, y: 22 }
const circuitHash = (cols: unknown[]): string => encodeURIComponent(JSON.stringify({ cols }))

const execModeProbePoints = (cssWidth: number): PixelSamplePoint[] => [
  { name: 'local', x: cssWidth - 100, y: 23 },
  { name: 'gpu', x: cssWidth - 30, y: 23 },
]

const switchToGpuMode = async (page: Page): Promise<void> => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  const points = execModeProbePoints(box.width)
  await page.mouse.click(box.x + points[1].x, box.y + points[1].y)
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const pixels = await sampleCanvasPixels(page, canvas, points)
    if (pixelRgbDistance(pixels.gpu, EXEC_MODE_GPU_FILL) < 36) return
    await page.waitForTimeout(50)
  }
  throw new Error('GPU mode did not reach expected fill')
}

const waitForExternalAmplitudeCell = async (page: Page, gateId: number, outcome: number) => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const cell = await readAmplitudeCell(page, gateId, outcome)
    if (cell && Math.round(cell.re * 100) / 100 === 0.75) return cell
    await page.waitForTimeout(50)
  }
  throw new Error(`External Amplitude cell ${gateId}:${outcome} did not become available`)
}

test('Run GPU uploads Qiskit Amplitude results into the display buffer', async ({ page }) => {
  await page.goto(`/#${circuitHash([['H'], ['Amps1']])}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await switchToGpuMode(page)
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    ;(window as any).__qniRunQiskitBackend = async (payloadJson: string) => {
      const payload = JSON.parse(payloadJson)
      ;(window as any).__qniLastQiskitRequest = payload
      const result = {
        status: 'completed',
        runner: 'test',
        qubits: payload.qubits,
        shots: payload.shots,
        histogram: { '0': 512, '1': 512 },
        amplitudes: [
          {
            gate_id: 2,
            span: 1,
            ket: [[0.25, 0], [0.75, 0]],
            incoherent: [0.25, 0.75],
            quality: 1,
            phase_lock_index: -1,
          },
        ],
        truncated: false,
      }
      ;(window as any).__qniLastQiskitResult = result
      return result
    }
  })

  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) throw new Error('expected egui canvas to be measurable')
  await page.mouse.click(box.x + RUN_GPU_BUTTON_POINT.x, box.y + RUN_GPU_BUTTON_POINT.y)
  await page.waitForFunction(() => (window as any).__qniLastQiskitResult?.status === 'completed')
  const cell = await waitForExternalAmplitudeCell(page, 2, 1)

  expect(Math.round(cell.re * 100) / 100).toBe(0.75)
})
