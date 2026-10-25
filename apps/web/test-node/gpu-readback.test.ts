import { test } from 'node:test'
import assert from 'node:assert/strict'
import { canaryViolations, decodeOutput } from '../test-gpu/rig'
import { rustConst } from '../test-gpu/app-source'

const CANARY = 0x7fbadbad
function blochOutput(slot: number, corruptGuard = false) {
  const offset = slot === 0 ? 0 : 4
  const bits = new Uint32Array(slot === 0 ? 8 : 12).fill(CANARY)
  new Float32Array(bits.buffer).set([0, 0, 0, -1], offset)
  if (corruptGuard) bits[offset + 4] = 0
  return decodeOutput('bloch', slot, offset, 4, bits.buffer)
}

for (const slot of [0, 1]) {
  test(`Bloch readback exposes only slot ${slot}, without guard NaNs`, () => {
    assert.deepEqual([...blochOutput(slot).values], [0, 0, 0, -1])
  })
  test(`Bloch readback preserves intact guards around slot ${slot}`, () => {
    assert.deepEqual(canaryViolations(blochOutput(slot)), [])
  })
  test(`Bloch readback detects nonfinite output in slot ${slot}`, () => {
    const output = blochOutput(slot)
    output.values[0] = NaN
    assert.deepEqual(canaryViolations(output), [
      `bloch byte ${output.offset * 4} slot ${slot}: nonfinite`,
    ])
  })
  test(`Bloch readback detects a write beyond slot ${slot}`, () => {
    assert.deepEqual(canaryViolations(blochOutput(slot, true)), [
      `bloch byte ${(slot === 0 ? 4 : 8) * 4} slot ${slot + 1}: canary overwritten`,
    ])
  })
}

test('probability readback excludes unused outcomes and neighboring slots', () => {
  const bytes = new Float32Array([-1, -1, NaN, NaN]).buffer
  assert.deepEqual([...decodeOutput('probability', 0, 0, 2, bytes).values], [-1, -1])
})

test('amplitude readback preserves the sparse incoherent output layout', () => {
  const outcomes = rustConst('MAX_AMPLITUDE_OUTCOMES')
  const stride = rustConst('AMPLITUDE_VALUES_PER_SLOT')
  const bits = new Uint32Array(stride * 3).fill(CANARY)
  const raw = new Float32Array(bits.buffer)
  raw.set([1, 0], stride)
  raw[stride + 2 * outcomes] = 1
  const output = decodeOutput('amplitude', 1, stride, 1, bits.buffer)
  assert.deepEqual({ length: output.values.length, coherent: [...output.values.slice(0, 2)],
    incoherent: output.values[2 * outcomes], guards: canaryViolations(output) },
  { length: stride, coherent: [1, 0], incoherent: 1, guards: [] })
})
