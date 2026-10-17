import { expect, test, type BrowserContext } from '@playwright/test'
import { readStateVector, waitForStartupReady } from './support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

type Column = (string | number)[]
type Parity = 'zpar' | 'xpar'

const basisInputs = (wires: number): number[][] =>
  Array.from({ length: 2 ** wires }, (_, input) =>
    Array.from({ length: wires }, (_, wire) => (input >> wire) & 1),
  )

const preparation = (bits: number[]): Column => bits.map((bit) => `|${bit}>`)
const label = (bits: number[]): string => bits.join(',')
const basisChange = (parity: Parity, wires: number): Column[] =>
  parity === 'xpar' ? [Array.from({ length: wires }, (_, wire) => wire < 2 ? 'H' : 1)] : []

const stateVectorOf = async (context: BrowserContext, cols: Column[]): Promise<number[]> => {
  const page = await context.newPage()
  try {
    await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols }))}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    await page.mouse.click(170 + 56 * (cols.length - 1), 224)
    await page.mouse.move(1000, 450)
    return (await readStateVector(page) as number[])
      .map((value) => Math.round(value * 1e4) / 1e4 || 0)
  } finally {
    await page.close()
  }
}

for (const parity of ['zpar', 'xpar'] as const) {
  for (const bits of basisInputs(3)) {
    test(`${parity} equals two CNOTs on basis [${label(bits)}]`, async ({ context }) => {
      const change = basisChange(parity, 3)
      const expected = await stateVectorOf(context, [
        preparation(bits),
        ...change,
        ['•', 1, 'X'],
        [1, '•', 'X'],
        ...change,
      ])

      expect(await stateVectorOf(context, [preparation(bits), [parity, parity, 'X']]))
        .toEqual(expected)
    })
  }

  for (const bits of basisInputs(4)) {
    test(`${parity} equals three CNOTs on basis [${label(bits)}]`, async ({ context }) => {
      const change: Column[] = parity === 'xpar' ? [['H', 'H', 'H', 1]] : []
      const expected = await stateVectorOf(context, [
        preparation(bits),
        ...change,
        ['•', 1, 1, 'X'],
        [1, '•', 1, 'X'],
        [1, 1, '•', 'X'],
        ...change,
      ])
      expect(await stateVectorOf(context, [preparation(bits), [parity, parity, parity, 'X']]))
        .toEqual(expected)
    })
  }

  for (const target of [0, 1]) {
    test(`${parity} does not apply with two satisfied controls and target ${target}`, async ({ context }) => {
      const prepared = [preparation([1, 1, target]), ...basisChange(parity, 3)]
      const expected = await stateVectorOf(context, prepared)

      expect(await stateVectorOf(context, [...prepared, [parity, parity, 'X']]))
        .toEqual(expected)
    })
  }

  for (const bits of basisInputs(4)) {
    const ordinary = bits[2] === 1 ? 'on' : 'off'
    test(`${parity} AND ordinary control ${ordinary} on basis [${label(bits)}]`, async ({ context }) => {
      const change = basisChange(parity, 4)
      const expected = await stateVectorOf(context, [
        preparation(bits),
        ...change,
        ['•', 1, '•', 'X'],
        [1, '•', '•', 'X'],
        ...change,
      ])

      expect(await stateVectorOf(context, [preparation(bits), [parity, parity, '•', 'X']]))
        .toEqual(expected)
    })
  }
}

for (const bits of basisInputs(3)) {
  test(`mixed X/Z parity equals a basis-changed XOR on basis [${label(bits)}]`, async ({ context }) => {
    const expected = await stateVectorOf(context, [
      preparation(bits),
      ['H', 1, 1],
      ['•', 1, 'X'],
      [1, '•', 'X'],
      ['H', 1, 1],
    ])

    expect(await stateVectorOf(context, [preparation(bits), ['xpar', 'zpar', 'X']]))
      .toEqual(expected)
  })
}
