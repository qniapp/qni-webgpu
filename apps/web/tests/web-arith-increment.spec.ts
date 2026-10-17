import { expect, test } from '@playwright/test'
import { waitForStartupReady, waitForStateVectorApprox } from './support/web-spec-helpers'

type Cols = (string | number)[][]
const basis = (qubits: number, index: number) => Array.from({ length: 2 ** qubits * 2 }, (_, i) => i === index * 2 ? 1 : 0)

for (const c of [
  { name: 'inc3 maps 5 to 6', cols: [['X', 1, 'X'], ['inc3']], qubits: 3, output: 6 },
  { name: 'inc3 wraps 7 to 0', cols: [['X', 'X', 'X'], ['inc3']], qubits: 3, output: 0 },
  { name: 'dec3 wraps 0 to 7', cols: [['dec3']], qubits: 3, output: 7 },
  { name: 'active control increments', cols: [[1, 1, 1, 'X'], ['inc3', 1, 1, '•']], qubits: 4, output: 9 },
  { name: 'inactive control preserves input', cols: [['inc3', 1, 1, '•']], qubits: 4, output: 0 },
  { name: 'active anti-control decrements', cols: [['dec3', 1, 1, '◦']], qubits: 4, output: 7 },
  { name: 'inactive anti-control preserves input', cols: [[1, 1, 1, 'X'], ['inc3', 1, 1, '◦']], qubits: 4, output: 8 },
  { name: 'odd external Z parity increments and uncomputes', cols: [[1, 1, 'X'], ['inc2', 1, 'zpar', 'zpar']], qubits: 4, output: 5 },
  { name: 'even external Z parity preserves input', cols: [[1, 1, 'X', 'X'], ['inc2', 1, 'zpar', 'zpar']], qubits: 4, output: 12 },
  { name: 'span-internal controls are ignored', cols: [['inc3', '•', '◦']], qubits: 3, output: 1 },
  { name: 'span-internal parity is ignored', cols: [['dec3', 'zpar', 'xpar']], qubits: 3, output: 7 },
] satisfies { name: string; cols: Cols; qubits: number; output: number }[]) {
  test(c.name, async ({ page }) => {
    await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: c.cols }))}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    await waitForStateVectorApprox(page, basis(c.qubits, c.output))
  })
}

test('increment preserves complex superposition amplitudes', async ({ page }) => {
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols: [['H'], ['S'], ['inc3']] }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const expected = Array(16).fill(0)
  expected[2] = Math.SQRT1_2
  expected[5] = Math.SQRT1_2
  await waitForStateVectorApprox(page, expected)
})

test('external X parity decrements a negative X eigenstate and uncomputes', async ({ page }) => {
  const cols: Cols = [[1, 1, 'X'], [1, 1, 'H'], ['dec2', 1, 'xpar', 'zpar']]
  await page.goto(`/#${encodeURIComponent(JSON.stringify({ cols }))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  const expected = Array(32).fill(0)
  expected[6] = Math.SQRT1_2
  expected[14] = -Math.SQRT1_2
  await waitForStateVectorApprox(page, expected)
})

test('arithmetic URL survives reload with Quirk family tokens', async ({ page }) => {
  const circuit = { cols: [['inc3'], ['dec2'], ['Amps3']] }
  await page.goto(`/#${encodeURIComponent(JSON.stringify(circuit))}`)
  await waitForStartupReady(page, { waitForStateVector: true })
  await page.reload()
  await waitForStartupReady(page, { waitForStateVector: true })
  expect(JSON.parse(decodeURIComponent(new URL(page.url()).hash.slice(1)))).toEqual(circuit)
})
