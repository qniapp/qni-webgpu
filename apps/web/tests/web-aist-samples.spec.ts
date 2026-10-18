import { expect, test, type Page } from '@playwright/test'
import {
  readAmplitudeCell, readBlochVectors, readMeasurementOutcomes,
  readProbabilityDistributions, readStateVector, waitForStartupReady,
} from './support/web-spec-helpers'

// 実際に組み込まれたサンプルを起動 URL で開く。JSON の複製を検証対象にしない。
const loadSample = async (page: Page, id: string): Promise<string> => {
  await page.goto('/')
  await waitForStartupReady(page)
  const json = await page.evaluate((sampleId) => {
    const library = JSON.parse((window as any).__qniCircuitPickerSnapshot())
    const entry = library.entries.find((item: { id: string }) => item.id === sampleId)
    if (!entry) throw new Error(`sample missing: ${sampleId}`)
    return entry.circuit_json as string
  }, id)
  await page.goto(`/#${encodeURIComponent(json)}`)
  await page.reload()
  await waitForStartupReady(page, { waitForStateVector: true })
  return json
}

const rounded = (value: number): number => {
  const result = Math.round(value * 10_000) / 10_000
  return result === 0 ? 0 : result
}
const orderedProbabilities = async (page: Page) =>
  (await readProbabilityDistributions(page)).sort((a, b) => a.gateId - b.gateId)

// マーカー列は ID を消費しない。表示の出現順から ID を得る。
const amplitudeIds = (json: string): number[] => {
  let id = 0
  const result: number[] = []
  for (const col of JSON.parse(json).cols as Array<Array<string | number>>) {
    if (col.length === 1 && typeof col[0] === 'string' && (col[0].startsWith('{') || col[0] === '}')) continue
    for (const token of col) {
      if (typeof token !== 'string') continue
      id += 1
      if (/^Amps\d+$/.test(token)) result.push(id)
    }
  }
  return result
}

for (const { id, qubits, outcomes } of [
  { id: 'bell', qubits: 2, outcomes: [0, 3] },
  { id: 'ghz', qubits: 3, outcomes: [0, 7] },
  { id: 'qft-4', qubits: 4, outcomes: [0, 8] },
]) {
  test(`${id}: final coherent superposition`, async ({ page }) => {
    await loadSample(page, id)
    const expected = Array<number>(2 ** (qubits + 1)).fill(0)
    for (const outcome of outcomes) expected[2 * outcome] = rounded(Math.SQRT1_2)
    await expect.poll(async () => (await readStateVector(page) as number[]).map(rounded)).toEqual(expected)
  })
}

for (const { id, outcome } of [{ id: 'bell', outcome: 3 }, { id: 'ghz', outcome: 7 }]) {
  test(`${id}: last amplitude display shows the all-one component`, async ({ page }) => {
    const json = await loadSample(page, id)
    const gateId = amplitudeIds(json).at(-1)
    if (gateId === undefined) throw new Error('amplitude display missing')
    await expect.poll(async () => rounded((await readAmplitudeCell(page, gateId, outcome))?.re ?? NaN)).toBe(rounded(Math.SQRT1_2))
  })
}

for (const { id, expected } of [
  { id: 'bell', expected: [[1, 0, 0]] },
  { id: 'ghz', expected: [[1, 0, 0]] },
  { id: 'quantum-teleportation', expected: [[Math.SQRT1_2, Math.SQRT1_2, 0], [Math.SQRT1_2, Math.SQRT1_2, 0]] },
]) {
  test(`${id}: Bloch displays explain state preparation and transfer`, async ({ page }) => {
    await loadSample(page, id)
    await expect.poll(async () => (await readBlochVectors(page))
      .sort((a, b) => a.gateId - b.gateId).map(({ x, y, z }) => [x, y, z].map(rounded)))
      .toEqual(expected.map(vector => vector.map(rounded)))
  })
}

for (const { id, slot, outcome, expected } of [
  { id: 'superdense-coding', slot: 0, outcome: 3, expected: 1 },
  { id: 'superdense-coding', slot: 1, outcome: 3, expected: 1 },
  { id: 'qft-4', slot: 0, outcome: 2, expected: 0.125 },
  { id: 'qft-4', slot: 1, outcome: 8, expected: 0.5 },
  { id: 'symmetry-breaking', slot: 0, outcome: 0, expected: 0.25 },
  { id: 'reversible-addition', slot: 2, outcome: 5, expected: 1 },
  { id: 'reversible-addition', slot: 3, outcome: 8, expected: 1 },
  { id: 'delayed-choice-eraser', slot: 3, outcome: 24, expected: 0 },
  { id: 'delayed-choice-eraser', slot: 1, outcome: 24, expected: 1 / 128 },
  { id: 'delayed-choice-eraser', slot: 4, outcome: 8, expected: 0 },
]) {
  test(`${id}: probability display ${slot + 1}, outcome ${outcome}`, async ({ page }) => {
    await loadSample(page, id)
    await expect.poll(async () => rounded((await orderedProbabilities(page))[slot]?.probabilities[outcome] ?? NaN)).toBe(rounded(expected))
  })
}

test('symmetry-breaking: joint distribution has only complementary pairs', async ({ page }) => {
  await loadSample(page, 'symmetry-breaking')
  await expect.poll(async () => {
    const values = (await orderedProbabilities(page))[2]?.probabilities
    return values ? [3, 6, 9, 12].reduce((sum, outcome) => sum + values[outcome], 0) : 0
  }).toBeGreaterThan(0.999)
})

test('symmetry-breaking: measured pairs always disagree', async ({ page }) => {
  await loadSample(page, 'symmetry-breaking')
  await expect.poll(async () => {
    const values = (await readMeasurementOutcomes(page)).sort((a, b) => a.gateId - b.gateId)
    return values.length === 4 && values[0].outcome !== values[2].outcome && values[1].outcome !== values[3].outcome
  }).toBe(true)
})

test('grover-search: each iteration increases the marked probability', async ({ page }) => {
  await loadSample(page, 'grover-search')
  await expect.poll(async () => {
    const values = (await orderedProbabilities(page)).map(entry => entry.probabilities[27])
    return values.slice(1).map((value, index) => value > values[index])
  }).toEqual([true, true, true, true])
})

test('grover-search: initial marked probability is uniform', async ({ page }) => {
  await loadSample(page, 'grover-search')
  await expect.poll(async () => (await orderedProbabilities(page))[0]?.probabilities[27]).toBeCloseTo(1 / 32, 4)
})

test('superdense-coding: final message is 11 on both sides regardless of global phase', async ({ page }) => {
  await loadSample(page, 'superdense-coding')
  await expect.poll(async () => {
    const state = await readStateVector(page) as number[]
    return (state[102] ?? 0) ** 2 + (state[103] ?? 0) ** 2
  }).toBeCloseTo(1, 4)
})

test('reversible-addition: final state is A=5 and B=8', async ({ page }) => {
  await loadSample(page, 'reversible-addition')
  const expected = Array<number>(2048).fill(0)
  expected[522] = 1
  await expect.poll(async () => (await readStateVector(page) as number[]).map(rounded)).toEqual(expected)
})

test('reversible-addition: labels survive the startup URL round trip', async ({ page }) => {
  await loadSample(page, 'reversible-addition')
  await expect.poll(async () => page.evaluate(() =>
    JSON.parse((window as any).__qniCircuitPickerSnapshot()).active_id)).toBe('reversible-addition')
})
