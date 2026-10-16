import { expect, test, type Page } from '@playwright/test'
import {
  readMeasurementOutcomes,
  readStateVector,
  waitForStartupReady,
} from './support/web-spec-helpers'

type Column = (string | number)[]
let circuitLoad = 0

const open = async (page: Page, cols: Column[]): Promise<void> => {
  await page.goto(`/?detector-test=${circuitLoad++}#${encodeURIComponent(JSON.stringify({ cols }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.mouse.move(1000, 600)
}

const sample = async (page: Page) => ({
  outcomes: (await readMeasurementOutcomes(page)).map(({ outcome }) => outcome),
  state: (await readStateVector(page) as number[]).map(value => {
    const rounded = Math.round(value * 1e4) / 1e4
    return rounded === 0 ? 0 : rounded
  }),
})

const sampleZeroAcrossSeeds = async (page: Page, detector: string) => {
  const results: { outcome: number, state: number[] }[] = []
  for (let seed = 0; seed < 32; seed++) {
    await open(page, [...Array.from({ length: seed }, () => ['…']), [detector]])
    const result = await sample(page)
    if (result.outcomes.length !== 1) throw new Error('Detector outcome unavailable')
    results.push({ outcome: result.outcomes[0], state: result.state })
  }
  return results
}

test('detector axis icons and GPU click digits', async ({ page }) => {
  await open(page, [
    ['H', 'H', 'H', 'H'],
    [1, 'Z', 1, 'Z'],
    [1, 1, 'S', 'S'],
    ['XDetector', 'XDetector', 'YDetector', 'YDetector'],
  ])
  await expect(page).toHaveScreenshot('detector-clicks.png', {
    clip: { x: 310, y: 240, width: 56, height: 224 },
  })
})

for (const axis of ['X', 'Y']) {
  const detector = `${axis}Detector`
  const phase: Column[] = axis === 'Y' ? [['S']] : []
  const positive = [0.7071, 0, ...(axis === 'Y' ? [0, 0.7071] : [0.7071, 0])]
  const negative = [0.7071, 0, ...(axis === 'Y' ? [0, -0.7071] : [-0.7071, 0])]

  test(`${detector} preserves its serialized id`, async ({ page }) => {
    await open(page, [[detector]])
    expect(JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1)))).toEqual({ cols: [[detector]] })
  })

  for (const outcome of [0, 1]) {
    const sign = outcome ? 'negative' : 'positive'
    const preparation: Column[] = [['H'], ...(outcome ? [['Z']] : []), ...phase]

    test(`${detector} ${sign} eigenstate has outcome ${outcome}`, async ({ page }) => {
      await open(page, [...preparation, [detector]])
      await expect.poll(async () => (await sample(page)).outcomes).toEqual([outcome])
    })

    test(`${detector} preserves the ${sign} eigenstate`, async ({ page }) => {
      await open(page, [...preparation, [detector]])
      await expect.poll(async () => (await sample(page)).state).toEqual(outcome ? negative : positive)
    })
  }

  test(`${detector} matches its explicit basis decomposition across seeds`, async ({ page }) => {
    test.setTimeout(180_000)
    const pairs = []
    for (let seed = 0; seed < 8; seed++) {
      const prefix: Column[] = [...Array.from({ length: seed }, () => ['…']), ['H'], ['T']]
      const before: Column[] = axis === 'X' ? [['H']] : [['S†'], ['H']]
      const after: Column[] = axis === 'X' ? [['H']] : [['H'], ['S']]
      await open(page, [...prefix, ...before.map(() => ['…']), [detector]])
      const actual = await sample(page)
      await open(page, [...prefix, ...before, ['Measure'], ...after])
      pairs.push({ actual, decomposed: await sample(page) })
    }
    expect(pairs.map(({ actual }) => actual)).toEqual(pairs.map(({ decomposed }) => decomposed))
  })

  for (const control of ['•', '◦']) {
    for (const outcome of [0, 1]) {
      test(`${detector} joint ${control} outcome ${outcome} preserves the correct subspace`, async ({ page }) => {
        test.setTimeout(180_000)
        const click = control === '•'
          ? [0, 0, 0.7071, 0, 0, 0, ...(axis === 'Y' ? [0, -0.7071] : [-0.7071, 0])]
          : [0.7071, 0, 0, 0, ...(axis === 'Y' ? [0, -0.7071] : [-0.7071, 0]), 0, 0]
        const noClick = control === '•'
          ? [0.8165, 0, 0.4082, 0, 0, 0, ...(axis === 'Y' ? [0, 0.4082] : [0.4082, 0])]
          : [0.4082, 0, 0.8165, 0, ...(axis === 'Y' ? [0, 0.4082] : [0.4082, 0]), 0, 0]
        let state: number[] | undefined
        for (let seed = 0; seed < 16; seed++) {
          await open(page, [...Array.from({ length: seed }, () => ['…']), ['H'], [control, detector]])
          const result = await sample(page)
          if (result.outcomes.length !== 1) throw new Error('Controlled detector outcome unavailable')
          if (result.outcomes[0] === outcome) {
            state = result.state
            break
          }
        }
        if (!state) throw new Error(`GPU seeds did not produce outcome ${outcome}`)
        expect(state).toEqual(outcome ? click : noClick)
      })
    }
  }

  test(`${detector} clicks approximately half the time on zero across GPU seeds`, async ({ page }) => {
    test.setTimeout(180_000)
    const results = await sampleZeroAcrossSeeds(page, detector)
    const clicks = results.filter(({ outcome }) => outcome === 1).length
    expect(Math.abs(clicks / results.length - 0.5)).toBeLessThanOrEqual(0.25)
  })

  test(`${detector} collapses zero to the sampled eigenstate across GPU seeds`, async ({ page }) => {
    test.setTimeout(180_000)
    const results = await sampleZeroAcrossSeeds(page, detector)
    expect(results.map(({ state }) => state)).toEqual(results.map(({ outcome }) => outcome ? negative : positive))
  })
}
