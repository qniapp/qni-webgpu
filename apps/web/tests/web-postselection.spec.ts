import { expect, test, type Page } from '@playwright/test'
import {
  readAmplitudeCell,
  readBlochVectors,
  readDensityMatrixCell,
  readMeasurementOutcomes,
  readProbabilityDistributions,
  waitForStartupReady,
} from './support/web-spec-helpers'

const openCircuit = async (page: Page, cols: unknown[][]) => {
  await page.goto('about:blank')
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
}

const readState = async (page: Page): Promise<number[]> => page.evaluate(async () => {
  const read = (window as unknown as { __eguiReadStateVector: () => Promise<number[]> }).__eguiReadStateVector
  return Array.from(await read()).map(value => Math.round(value * 1e5) / 1e5)
})

const readRates = async (page: Page): Promise<number[]> => page.evaluate(async () => {
  const read = (window as unknown as { __eguiReadPostselectionRates?: () => Promise<number[]> }).__eguiReadPostselectionRates
  if (!read) throw new Error('postselection rate readback hook is missing')
  return Array.from(await read()).map(value => Math.round(value * 1e5) / 1e5)
})

const branches = [
  { token: '|0⟩⟨0|', before: 'H', state: [1, 0, 0, 0], rate: 0.5 },
  { token: '|1⟩⟨1|', before: 'H', state: [0, 0, 1, 0], rate: 0.5 },
  { token: '|+⟩⟨+|', before: 'H', state: [0.70711, 0, 0.70711, 0], rate: 1 },
  { token: '|-⟩⟨-|', before: 'X', state: [-0.70711, 0, 0.70711, 0], rate: 0.5 },
]

for (const branch of branches) {
  test(`${branch.token} produces a conditional state on GPU`, async ({ page }) => {
    await openCircuit(page, [[branch.before], [branch.token]])
    await expect.poll(() => readState(page)).toEqual(branch.state)
  })

  test(`${branch.token} retains its acceptance probability on GPU`, async ({ page }) => {
    await openCircuit(page, [[branch.before], [branch.token]])
    await expect.poll(() => readRates(page)).toEqual([2, branch.rate])
  })
}

test('impossible postselection leaves an empty state instead of stale amplitudes', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|']])
  await expect.poll(() => readState(page)).toEqual([0, 0, 0, 0])
})

test('impossible postselection reports zero acceptance', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|']])
  await expect.poll(() => readRates(page)).toEqual([2, 0])
})

test('postselecting q0 conditions the other half of a Bell pair', async ({ page }) => {
  await openCircuit(page, [['H'], ['•', 'X'], ['|1⟩⟨1|']])
  await expect.poll(() => readState(page)).toEqual([0, 0, 0, 0, 0, 0, 1, 0])
})

for (const [token, zToken] of [['|+⟩⟨+|', '|0⟩⟨0|'], ['|-⟩⟨-|', '|1⟩⟨1|']]) {
  test(`${token} agrees with H projector H`, async ({ page }) => {
    await openCircuit(page, [['H'], ['S'], [token]])
    const direct = await readState(page)
    await openCircuit(page, [['H'], ['S'], ['H'], [zToken], ['H']])
    await expect.poll(() => readState(page)).toEqual(direct)
  })
}

test('successive postselections retain cumulative survival', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['H'], ['|1⟩⟨1|']])
  await expect.poll(() => readRates(page)).toEqual([2, 0.5, 4, 0.25])
})

test('an unchanged second projector retains the first column survival', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['H'], ['|+⟩⟨+|']])
  await expect.poll(() => readRates(page)).toEqual([2, 0.5, 4, 0.5])
})

const survivalLabelCases = [
  { name: 'omits-50-kept-50', cols: [['H'], ['|0⟩⟨0|']] },
  { name: 'unchanged-second-column', cols: [['H'], ['|0⟩⟨0|'], ['H'], ['|+⟩⟨+|']] },
  { name: 'omits-100-kept-0', cols: [['X'], ['|0⟩⟨0|']] },
  { name: 'no-loss-no-labels', cols: [['H'], ['|+⟩⟨+|']] },
  { name: 'same-column-omits-75', cols: [['H', 'H'], ['|0⟩⟨0|', '|1⟩⟨1|']] },
  { name: 'after-zero-no-extra-label', cols: [['X'], ['|0⟩⟨0|'], ['|0⟩⟨0|']] },
  { name: 'omits-over-99-kept-under-1', cols: [['Ry(π/32)'], ['|1⟩⟨1|']] },
  { name: 'nonzero-below-f32-subtraction-precision', cols: [['Ry(π/16)', 'Ry(π/16)', 'Ry(π/16)', 'Ry(π/16)'], ['|1⟩⟨1|', '|1⟩⟨1|', '|1⟩⟨1|', '|1⟩⟨1|']] },
  { name: 'tiny-loss-no-labels', cols: [['Ry(π/32)'], ['|0⟩⟨0|']] },
  { name: 'small-loss-only-omits', cols: [['Ry(π/16)'], ['|0⟩⟨0|']] },
]

for (const { name, cols } of survivalLabelCases) {
  test(`GPU survival labels: ${name}`, async ({ page }) => {
    await openCircuit(page, cols)
    await page.mouse.move(999, 450)
    await expect(page).toHaveScreenshot(`postselection-${name}.png`)
  })
}

test('state panel occludes column survival labels when moved over the circuit', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|']])
  await page.mouse.move(500, 552)
  await page.mouse.down()
  await page.mouse.move(400, 344, { steps: 12 })
  await page.mouse.up()
  await page.mouse.move(999, 450)
  await expect(page).toHaveScreenshot('postselection-panel-occludes-labels.png')
})

test('Bloch display captures the conditional state', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['Bloch']])
  await expect.poll(async () => (await readBlochVectors(page)).map(({ gateId, x, y, z }) => [gateId, x + 0, y + 0, z])).toEqual([[3, 0, 0, 1]])
})

test('Probability display captures the conditional distribution', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['Probability']])
  await expect.poll(async () => (await readProbabilityDistributions(page))[0]?.probabilities.slice(0, 2)).toEqual([1, 0])
})

test('Amplitude display captures normalized conditional amplitudes', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['Amps1']])
  await expect.poll(async () => {
    const cell = await readAmplitudeCell(page, 3, 0)
    return cell && [cell.re, cell.im, cell.incoherent]
  }).toEqual([1, 0, 1])
})

test('Density display captures normalized conditional density', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['Density']])
  await expect.poll(async () => {
    const cell = await readDensityMatrixCell(page, 3, 0, 0)
    return cell && [cell.re, cell.im, Math.round(cell.unity * 1e5) / 1e5]
  }).toEqual([1, 0, 1])
})

test('Probability display marks an impossible conditional distribution undefined', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Probability']])
  await expect.poll(async () => (await readProbabilityDistributions(page))[0]?.probabilities.slice(0, 2)).toEqual([-1, -1])
})

test('Density display marks an impossible conditional density empty', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Density']])
  await expect.poll(async () => {
    const cell = await readDensityMatrixCell(page, 3, 0, 0)
    return cell && [cell.re, cell.im, cell.unity]
  }).toEqual([0, 0, Math.fround(1e-12)])
})

test('measurement after postselection samples the normalized conditional state', async ({ page }) => {
  await openCircuit(page, [['H'], ['|1⟩⟨1|'], ['Measure']])
  await expect.poll(() => readState(page)).toEqual([0, 0, 1, 0])
})

test('measurement after impossible postselection cannot revive a state', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Measure']])
  await expect.poll(() => readState(page)).toEqual([0, 0, 0, 0])
})

test('measurement after impossible postselection has no defined outcome', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Measure>a'], ['X<a']])
  await expect.poll(() => readMeasurementOutcomes(page)).toEqual([{ gateId: 3, outcome: -1 }])
})

test('selecting a column before impossible postselection restores its defined snapshot', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|']])
  await page.mouse.click(170, 224)
  await page.mouse.move(1000, 450)
  await expect.poll(() => readState(page)).toEqual([0, 0, 1, 0])
})

test('column preview does not change gate survival rates', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|']])
  await page.mouse.click(170, 224)
  await page.mouse.move(1000, 450)
  await expect.poll(async () => ({ state: await readState(page), rates: await readRates(page) })).toEqual({
    state: [0.70711, 0, 0.70711, 0],
    rates: [2, 0.5],
  })
})

test('same-column projectors multiply their survival probabilities', async ({ page }) => {
  await openCircuit(page, [['H', 'H'], ['|0⟩⟨0|', '|1⟩⟨1|']])
  await expect.poll(() => readRates(page)).toEqual([3, 0.5, 4, 0.25])
})

test('orthogonal X postselection is impossible', async ({ page }) => {
  await openCircuit(page, [['H'], ['|-⟩⟨-|']])
  await expect.poll(() => readState(page)).toEqual([0, 0, 0, 0])
})

test('later writes and projectors cannot revive an impossible state', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['|1>'], ['Measure'], ['|0>'], ['|+⟩⟨+|']])
  await expect.poll(() => readRates(page)).toEqual([2, 0, 6, 0])
})

test('controlled projection preserves the inactive slice before normalization', async ({ page }) => {
  await openCircuit(page, [['H', 'H'], ['•', '|0⟩⟨0|']])
  await expect.poll(() => readState(page)).toEqual([0.57735, 0, 0.57735, 0, 0.57735, 0, 0, 0])
})

test('anti-controlled projection preserves the inactive slice before normalization', async ({ page }) => {
  await openCircuit(page, [['H', 'H'], ['◦', '|0⟩⟨0|']])
  await expect.poll(() => readState(page)).toEqual([0.57735, 0, 0.57735, 0, 0, 0, 0.57735, 0])
})

test('controlled projection retains its global acceptance probability', async ({ page }) => {
  await openCircuit(page, [['H', 'H'], ['•', '|0⟩⟨0|']])
  await expect.poll(() => readRates(page)).toEqual([4, 0.75])
})

test('postselection normalization covers more than one reduction stride', async ({ page }) => {
  await openCircuit(page, [['H', 'H', 'H', 'H', 'H', 'H', 'H'], [1, 1, 1, 1, 1, 1, '|0⟩⟨0|']])
  const expected = [...Array.from({ length: 64 }, () => [0.125, 0]).flat(), ...Array<number>(128).fill(0)]
  await expect.poll(() => readState(page)).toEqual(expected)
})

test('postselection survival covers more than one reduction stride', async ({ page }) => {
  await openCircuit(page, [['H', 'H', 'H', 'H', 'H', 'H', 'H'], [1, 1, 1, 1, 1, 1, '|0⟩⟨0|']])
  await expect.poll(() => readRates(page)).toEqual([8, 0.5])
})

test('a defined column preview clears the undefined panel rendering', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Bloch'], ['Probability'], ['Amps1'], ['Density']])
  await page.mouse.click(170, 224)
  await page.mouse.move(999, 450)
  await expect(page).toHaveScreenshot('postselection-defined-preview.png')
})

test('conditional displays and GPU survival label render together', async ({ page }) => {
  await openCircuit(page, [['H'], ['|0⟩⟨0|'], ['Bloch'], ['Probability'], ['Amps1'], ['Density']])
  await page.mouse.move(999, 450)
  await expect(page).toHaveScreenshot('postselection-conditional.png')
})

test('undefined displays and zero survival render explicitly', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Bloch'], ['Probability'], ['Amps1'], ['Density']])
  await page.mouse.move(999, 450)
  await expect(page).toHaveScreenshot('postselection-undefined.png')
})

test('undefined measurement omits the result and leaves classical control inactive', async ({ page }) => {
  await openCircuit(page, [['X'], ['|0⟩⟨0|'], ['Measure>a'], ['X<a']])
  await page.mouse.move(999, 450)
  await expect(page).toHaveScreenshot('postselection-undefined-measurement.png')
})
