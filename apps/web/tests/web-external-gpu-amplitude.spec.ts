import { expect, test, type Page } from '@playwright/test'
import {
  pixelRgbDistance,
  readAmplitudeCell,
  readBlochVectors,
  readProbabilityDistributions,
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

const waitForExternalAmplitudeCell = async (
  page: Page,
  gateId: number,
  outcome: number,
  expectedRe = 0.75,
) => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const cell = await readAmplitudeCell(page, gateId, outcome)
    if (cell && Math.round(cell.re * 100) / 100 === expectedRe) return cell
    await page.waitForTimeout(50)
  }
  throw new Error(`External Amplitude cell ${gateId}:${outcome} did not become available`)
}

const waitForExternalBlochVector = async (page: Page, gateId: number, expectedX = 0.5) => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const vector = (await readBlochVectors(page)).find((entry) => entry.gateId === gateId)
    if (vector && Math.round(vector.x * 100) / 100 === expectedX) return vector
    await page.waitForTimeout(50)
  }
  throw new Error(`External Bloch vector ${gateId} did not become available`)
}

const waitForExternalProbability = async (
  page: Page,
  gateId: number,
  outcome: number,
  expectedProbability: number,
) => {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const entry = (await readProbabilityDistributions(page)).find((item) => item.gateId === gateId)
    const value = entry?.probabilities[outcome]
    if (value !== undefined && Math.round(value * 100) / 100 === expectedProbability) return value
    await page.waitForTimeout(50)
  }
  throw new Error(`External Probability value ${gateId}:${outcome} did not become available`)
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

test('Run GPU uploads Qiskit Probability results into the display buffer', async ({ page }) => {
  await page.goto(`/#${circuitHash([['H'], ['Probability']])}`)
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
        probability: [{ gate_id: 2, span: 1, probabilities: [0.25, 0.75] }],
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
  const probability = await waitForExternalProbability(page, 2, 1, 0.75)

  expect(Math.round(probability * 100) / 100).toBe(0.75)
})

test('Run GPU uploads combined Qiskit display results into GPU buffers', async ({ page }) => {
  await page.goto(`/#${circuitHash([['H', 1, 1], ['Amps1', 'Bloch', 'Probability']])}`)
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
        histogram: { '000': 512, '100': 512 },
        amplitudes: [
          {
            gate_id: 2,
            span: 1,
            ket: [[0.4, 0], [0.6, 0]],
            incoherent: [0.4, 0.6],
            quality: 1,
            phase_lock_index: -1,
          },
        ],
        bloch: [{ gate_id: 3, vector: [0.4, 0.2, -0.4] }],
        probability: [{ gate_id: 4, span: 1, probabilities: [0.7, 0.3] }],
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
  const cell = await waitForExternalAmplitudeCell(page, 2, 1, 0.6)
  const vector = await waitForExternalBlochVector(page, 3, 0.4)
  const probability = await waitForExternalProbability(page, 4, 1, 0.3)

  expect({
    re: Math.round(cell.re * 100) / 100,
    x: Math.round(vector.x * 100) / 100,
    p: Math.round(probability * 100) / 100,
  }).toEqual({ re: 0.6, x: 0.4, p: 0.3 })
})

test('Run GPU uploads Qiskit Bloch results into the display buffer', async ({ page }) => {
  await page.goto(`/#${circuitHash([['H'], ['Bloch']])}`)
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
        bloch: [{ gate_id: 2, vector: [0.5, 0.25, -0.5] }],
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
  const vector = await waitForExternalBlochVector(page, 2)

  expect(Math.round(vector.x * 100) / 100).toBe(0.5)
})
