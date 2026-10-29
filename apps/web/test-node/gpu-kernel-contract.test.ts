import { test } from 'node:test'
import assert from 'node:assert/strict'
import { checkDrift, pack, rustConst } from '../test-gpu/app-source'
import { aggregateInput, aggregateInstances, matrices } from '../test-gpu/cases'
import {
  aggregateExpectedRows,
  amplitude,
  bloch,
  density,
  gate,
  probability,
  rand,
} from '../test-gpu/reference'
test('Rust/WGSL host contract', () => {
  assert.doesNotThrow(checkDrift)
})
test('Rust maximum state count', () => {
  assert.equal(rustConst('MAX_STATE_COUNT'), 65536)
})
test('gate parameters occupy 64 bytes', () => {
  assert.equal(
    pack('GateParams', {
      m00: [1, 0],
      m01: [0, 0],
      m10: [0, 0],
      m11: [1, 0],
      bit: 0,
      state_count: 2,
      control_mask: 0,
      control_value: 0,
      mode: 0,
      condition_slot: 0,
    }).byteLength,
    64,
  )
})
for (const instance of aggregateInstances) {
  test(`aggregate slot ${instance.slot} avoids near-integer sampled row boundaries`, () => {
    assert.doesNotThrow(() => aggregateExpectedRows(instance, aggregateInput))
  })
}
test('aggregate rejects near-integer sampled row boundaries', () => {
  assert.throws(
    () => aggregateExpectedRows({ ...aggregateInstances[4], rect_size: [20, 879.5] }, aggregateInput),
    /case error aggregate slot 10: row 298 near boundary/,
  )
})

for (const [i, expected] of [0, 1, 0].entries()) {
  test(`fixed Bloch component ${i}`, () => {
    const plusI: [number, number][] = [
      [Math.SQRT1_2, 0],
      [0, Math.SQRT1_2],
    ]
    assert.ok(Math.abs(bloch(plusI, 0)[i] - expected) < 1e-14)
  })
}
for (const i of [0, 1]) {
  test(`fixed Hadamard probability ${i}`, () => {
    const h = gate(
      [
        [1, 0],
        [0, 0],
      ],
      0,
      matrices.H,
    )
    assert.ok(Math.abs(probability(h, 0, 1)[i] - 0.5) < 1e-14)
  })
}
const ghz: [number, number][] = [
  [Math.SQRT1_2, 0],
  [0, 0],
  [0, 0],
  [Math.SQRT1_2, 0],
]
for (const i of [0, 3, 12, 15]) {
  test(`fixed GHZ density entry ${i}`, () => {
    assert.ok(Math.abs(density(ghz, 0, 2).data[i][0] - 0.5) < 1e-14)
  })
}
test('fixed GHZ density metadata', () => {
  assert.ok(Math.abs(density(ghz, 0, 2).meta - 1) < 1e-14)
})
test('fixed basis amplitude metadata', () => {
  assert.equal(
    amplitude(
      [
        [1, 0],
        [0, 0],
      ],
      0,
      1,
    ).meta[2],
    1,
  )
})
const hand = amplitude(
  [[0, Math.SQRT1_2], [Math.SQRT1_2, 0], [0, 0], [0, 0]], 0, 1,
)
const expectedKet = [[Math.SQRT1_2, 0], [0, -Math.SQRT1_2]]
for (const [i, value] of hand.ket.entries()) {
  for (const [j, component] of value.entries()) {
    test(`fixed amplitude component ${i},${j}`, () => {
      assert.ok(Math.abs(component - expectedKet[i][j]) < 1e-14)
    })
  }
}
for (const [i, value] of hand.meta.entries()) {
  test(`fixed amplitude metadata ${i}`, () => {
    assert.ok(Math.abs(value - [1, 0, 1, 0][i]) < 1e-14)
  })
}
for (const [seed, expected] of [[0, 0.3175988495349884], [1, 0.238451287150383], [42, 0.42170950770378113]]) {
  test(`fixed random value for seed ${seed}`, () => {
    assert.equal(rand(seed), expected)
  })
}

test('measurement reduction packs joint controls into a padded 32-byte uniform', () => {
  assert.deepEqual(
    Array.from(new Uint32Array(pack('MeasureReduceParams', {
      qubit_bit: 2,
      state_count: 8,
      output_slot: 3,
      seed: 7,
      control_mask: 3,
      control_value: 1,
    }))),
    [2, 8, 3, 7, 3, 1, 0, 0],
  )
})

test('measurement collapse packs joint controls into a padded 32-byte uniform', () => {
  assert.deepEqual(
    Array.from(new Uint32Array(pack('MeasureCollapseParams', {
      qubit_bit: 2,
      state_count: 8,
      aux_slot: 3,
      control_mask: 3,
      control_value: 1,
    }))),
    [2, 8, 3, 0, 3, 1, 0, 0],
  )
})
