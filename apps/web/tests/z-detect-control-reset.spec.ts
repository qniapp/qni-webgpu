import { test, expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import {
  readMeasurementOutcomes,
  readStateVector,
  waitForStartupReady,
} from './support/web-spec-helpers'

type Column = (string | number)[]

const loadCircuit = async (page: Page, cols: Column[]): Promise<number[]> => {
  await page.goto('about:blank')
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const values = await readStateVector(page) as number[]
  return Array.from({ length: values.length / 2 }, (_, index) =>
    Math.round((values[2 * index] ** 2 + values[2 * index + 1] ** 2) * 1e6) / 1e6,
  )
}

const basisProbabilities = (qubits: number, basis: number): number[] =>
  Array.from({ length: 2 ** qubits }, (_, index) => Number(index === basis))

test('WebGPU ZDetectControlReset controls X with its column-start H sample and resets its wire', async ({ page }) => {
  const probabilities = await loadCircuit(page, [['H'], ['ZDetectControlReset', 'X']])
  const outcomes = await readMeasurementOutcomes(page)
  const sample = outcomes[0]?.outcome ?? -1

  expect({
    count: outcomes.length,
    validSample: sample === 0 || sample === 1,
    probabilities,
  }).toEqual({
    count: 1,
    validSample: true,
    probabilities: basisProbabilities(2, 2 * sample),
  })
})

for (const input of [0, 1]) {
  test(`WebGPU ZDetectControlReset on |${input}> matches control-X then Measure and Write0`, async ({ page }) => {
    const preparation: Column = [input ? 'X' : '|0>']
    const detector = await loadCircuit(page, [preparation, ['ZDetectControlReset', 'X']])
    const decomposition = await loadCircuit(page, [
      preparation,
      ['•', 'X'],
      ['Measure'],
      ['|0>'],
    ])
    const expected = basisProbabilities(2, 2 * input)

    expect({ detector, decomposition }).toEqual({ detector: expected, decomposition: expected })
  })

  test(`WebGPU ZDetectControlReset on |${input}> leaves later-column X unconditional`, async ({ page }) => {
    const probabilities = await loadCircuit(page, [
      [input ? 'X' : '|0>'],
      ['ZDetectControlReset', 'X'],
      ['X', 'X'],
    ])

    expect(probabilities).toEqual(basisProbabilities(2, 1 + 2 * (1 - input)))
  })
}

for (const { inputs, control, value } of [
  { inputs: [1, 1], control: '•', value: 1 },
  { inputs: [1, 1], control: '•', value: 0 },
  { inputs: [1, 0], control: '•', value: 1 },
  { inputs: [0, 1], control: '•', value: 1 },
  { inputs: [1, 1], control: '◦', value: 0 },
  { inputs: [1, 1], control: '◦', value: 1 },
]) {
  test(`WebGPU detectors ${inputs.join('')} combine with ${control}=${value} and reset unconditionally`, async ({ page }) => {
    const preparation: Column = [...inputs, value].map(bit => bit ? 'X' : '|0>')
    const detector = await loadCircuit(page, [
      preparation,
      ['ZDetectControlReset', 'ZDetectControlReset', control, 'X'],
    ])
    const decomposition = await loadCircuit(page, [
      preparation,
      ['•', '•', control, 'X'],
      ['Measure', 'Measure'],
      ['|0>', '|0>'],
    ])
    const enabled = inputs.every(bit => bit === 1) && value === (control === '•' ? 1 : 0)
    const expected = basisProbabilities(4, 4 * value + (enabled ? 8 : 0))

    expect({ detector, decomposition }).toEqual({ detector: expected, decomposition: expected })
  })
}
