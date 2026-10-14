import { test, expect } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import path from 'node:path'

const assetOrigin = 'http://localhost:4175/tutorial/assets/'

test('native QFT and QFT dagger match every symmetric DFT column on WebGPU', async ({ page }) => {
  await page.route(`${assetOrigin}*`, async route => {
    const file = new URL(route.request().url()).pathname.split('/').pop()!
    await route.fulfill({
      body: await readFile(path.resolve(__dirname, '../embed-dist', file)),
      contentType: file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript',
      headers: { 'Access-Control-Allow-Origin': '*' },
    })
  })
  await page.route('**/host/qft', route => route.fulfill({
    contentType: 'text/html',
    body: '<html><body><canvas style="width:960px;height:640px"></canvas></body></html>',
  }))
  await page.goto('/host/qft')
  const columns = await page.evaluate(async origin => {
    const { prepareEmbed } = await import(`${origin}qni-embed.mjs`)
    await prepareEmbed()
    const { start_embed } = await import(`${origin}qni-web.js`)
    const canvas = document.querySelector('canvas')!
    const columns: { n: number, dagger: boolean, c: number, values: number[] }[] = []
    for (const n of [2, 3]) {
      for (const dagger of [false, true]) {
        for (let c = 0; c < 2 ** n; c++) {
          // Wire q0 is bit 0 (LSB), as in WireIndex::to_qubit_bit.
          const input = Array.from({ length: n }, (_, wire) => c & (1 << wire) ? 'X' : 1)
          const circuit = JSON.stringify({ cols: [input, [`QFT${dagger ? '†' : ''}${n}`]] })
          const runner = await start_embed(canvas, circuit, true, undefined, undefined, false)
          try {
            runner.selectStep(runner.stepCount() - 1)
            columns.push({ n, dagger, c, values: Array.from(await runner.read_state_vector()) })
          } finally {
            runner.destroy()
            runner.free()
          }
        }
      }
    }
    return columns
  }, assetOrigin)

  const failures: string[] = []
  for (const { n, dagger, c, values } of columns) {
    const dim = 2 ** n
    const label = `QFT${dagger ? '†' : ''}${n}, c=${c}`
    if (values.length !== 2 * dim) failures.push(`${label}: length ${values.length}, expected ${2 * dim}`)
    // Quirk FourierTransformGates uses + for QFT and - for its adjoint.
    const sign = dagger ? -1 : 1
    for (let r = 0; r < dim; r++) {
      const theta = sign * 2 * Math.PI * r * c / dim
      const expected = [Math.cos(theta) / Math.sqrt(dim), Math.sin(theta) / Math.sqrt(dim)]
      for (const component of [0, 1]) {
        const actual = values[2 * r + component]
        if (!Number.isFinite(actual) || Math.abs(actual - expected[component]) > 1e-5) {
          failures.push(`${label}, r=${r}, ${component === 0 ? 're' : 'im'}: ${actual}, expected ${expected[component]}`)
        }
      }
    }
  }
  expect(failures).toEqual([])
})
