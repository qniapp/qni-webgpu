import { test, expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import { readMeasurementOutcomes, readStateVector, waitForStartupReady } from './support/web-spec-helpers'

type Column = (string | number)[]

const stateAfter = async (page: Page, cols: Column[]): Promise<number[]> => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  return await readStateVector(page) as number[]
}

const probabilities = (state: number[]): number[] =>
  Array.from({ length: state.length / 2 }, (_, i) =>
    Math.round((state[2 * i] ** 2 + state[2 * i + 1] ** 2) * 1e6) / 1e6,
  )

test('Write0 resets H superposition with probability one', async ({ page }) => {
  expect(probabilities(await stateAfter(page, [['H'], ['|0>']]))).toEqual([1, 0])
})

test('Write0 samples a Bell branch rather than merging its amplitude pairs', async ({ page }) => {
  const result = probabilities(await stateAfter(page, [['H'], ['•', 'X'], ['|0>']]))
  expect([[1, 0, 0, 0], [0, 0, 1, 0]]).toContainEqual(result)
})

test('Write0 keeps its internal sample out of visible measurement outcomes', async ({ page }) => {
  await stateAfter(page, [['H'], ['|0>'], ['Measure']])
  expect((await readMeasurementOutcomes(page)).map(result => result.outcome)).toEqual([0])
})

test('Write0 leaves only the original flagged measurement visible', async ({ page }) => {
  await stateAfter(page, [['H'], ['Measure>a'], ['|0>'], ['X<a']])
  expect(await readMeasurementOutcomes(page)).toHaveLength(1)
})

test('Write0 preserves a previously recorded measurement flag', async ({ page }) => {
  const state = await stateAfter(page, [['H'], ['Measure>a'], ['|0>'], ['X<a']])
  const outcome = (await readMeasurementOutcomes(page))[0]?.outcome
  expect(probabilities(state)).toEqual(outcome === 0 ? [1, 0] : outcome === 1 ? [0, 1] : [])
})

// WRITE instruction page: normalize (-0.38268, 0.7855 - 0.48636i).
// Ry prepares the magnitudes and P prepares their relative phase;
// the omitted global phase cannot affect the reset probability.
const tutorialAlpha = -0.38268
const tutorialBeta = { re: 0.7855, im: -0.48636 }
const tutorialNorm = Math.hypot(tutorialAlpha, tutorialBeta.re, tutorialBeta.im)
const tutorialTheta = 2 * Math.acos(Math.abs(tutorialAlpha) / tutorialNorm)
const tutorialPhase = Math.atan2(tutorialBeta.im, tutorialBeta.re) - Math.PI

const piFraction = (radians: number): string => `${Math.round(radians / Math.PI * 10_000_000)}π/10000000`
const tutorialPreparation: Column[] = [
  [`Ry(${piFraction(tutorialTheta)})`],
  [`P(${piFraction(tutorialPhase)})`],
]

test('WRITE tutorial preparation matches the printed arbitrary-amplitude probabilities', async ({ page }) => {
  const state = await stateAfter(page, tutorialPreparation)
  expect(probabilities(state)).toEqual([0.146444, 0.853556])
})

test('WRITE tutorial arbitrary-amplitude example resets to the north pole', async ({ page }) => {
  const state = await stateAfter(page, [...tutorialPreparation, ['|0>']])
  expect(probabilities(state)).toEqual([1, 0])
})

for (const control of ['•', '◦']) {
  test(`Write0 with inactive ${control} preserves target superposition`, async ({ page }) => {
    const state = await stateAfter(page, [
      [control === '•' ? 1 : 'X', 'H'],
      [control, '|0>'],
    ])
    expect(probabilities(state)).toEqual(control === '•' ? [0.5, 0, 0.5, 0] : [0, 0.5, 0, 0.5])
  })

  test(`Write0 with quantum ${control} preserves the complex no-click subspace`, async ({ page }) => {
    const state = await stateAfter(page, [
      ['H', 'H'],
      [1, 'P(π/2)'],
      [control, '|0>'],
    ])
    const amplitude = Math.round(1 / Math.sqrt(3) * 1e5) / 1e5
    const noClick = control === '•'
      ? [amplitude, 0, amplitude, 0, 0, amplitude, 0, 0]
      : [amplitude, 0, amplitude, 0, 0, 0, 0, amplitude]
    // The standalone seed and these gate IDs deterministically choose no-click.
    const rounded = state.map(value => Math.round(value * 1e5) / 1e5 || 0)
    expect(rounded).toEqual(noClick)
  })

  test(`Write0 with active ${control} resets target superposition`, async ({ page }) => {
    const state = await stateAfter(page, [
      [control === '•' ? 'X' : 1, 'H'],
      [control, '|0>'],
    ])
    expect(probabilities(state)).toEqual(control === '•' ? [0, 1, 0, 0] : [1, 0, 0, 0])
  })
}
