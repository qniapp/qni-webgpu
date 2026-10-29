import { test } from 'node:test'
import assert from 'node:assert/strict'
import { openGpu } from './gpu'
import { createRig } from './rig'
import { aggregateInput, aggregateInstances, cases, reset, matrices, type RecomputeCase } from './cases'
import * as ref from './reference'
import { compare } from './check'
import { rustConst, wgsl } from './app-source'
const kernels = [
  'state_compute',
  'measure_reduce',
  'measure_collapse',
  'probability_reduce',
  'probability_normalize',
  'probability_aggregate',
  'bloch_reduce',
  'amplitude_capture',
  'density_capture',
] as const
function expected(c: RecomputeCase) {
  let state: ref.C[] =
    c.init === 'ground'
      ? Array.from({ length: 1 << c.qubits }, (_, i) => (i === 0 ? [1, 0] : [0, 0]))
      : c.init.map(([a, b]) => [a, b])
  const outputs: { name: string; slot: number; data: number[]; categorical?: number[] }[] = []
  for (const op of c.ops) {
    const mask = 'controls' in op ? (op.controls?.mask ?? 0) : 0
    const value = 'controls' in op ? (op.controls?.value ?? 0) : 0
    if (mask) {
      const trace = state.reduce((p, a, i) => p + ((i & mask) === value ? ref.mag(a) : 0), 0)
      if (trace > 0 && trace < 1e-2)
        throw Error(`case error ${c.name}: ill-conditioned control trace ${trace}`)
    }
    switch (op.kind) {
      case 'gate': {
        if (op.condition !== undefined) {
          const aux = [...outputs]
            .reverse()
            .find((x) => x.name === 'measurement' && x.slot === op.condition)!.data
          if (aux[2] < 0.5) break
        }
        if (op.mode)
          for (let pair = 0; pair < state.length / 2; pair++) {
            const i0 = ((pair >> op.bit) << (op.bit + 1)) | (pair & ((1 << op.bit) - 1))
            if ((i0 & mask) !== value) continue
            const difference = Math.abs(ref.mag(state[i0 | (1 << op.bit)]) - ref.mag(state[i0]))
            if (Math.abs(difference - 1e-6) <= 5e-7)
              throw Error(`case error ${c.name}: Write decision near epsilon at pair ${pair}`)
          }
        state = ref.gate(state, op.bit, op.m, mask, value, op.mode)
        break
      }
      case 'measure': {
        const result = ref.measure(state, op.bit, op.gateId, mask, value)
        if (Math.abs(result.aux[1] - result.aux[0]) <= 1e-4)
          throw Error(`case error ${c.name}: sample near p0`)
        outputs.push({ name: 'measurement', slot: op.slot, data: result.aux, categorical: [2] })
        break
      }
      case 'collapse': {
        const aux = [...outputs]
          .reverse()
          .find((x) => x.name === 'measurement' && x.slot === op.auxSlot)!.data
        state = ref.collapse(state, op.bit, aux, mask, value)
        break
      }
      case 'bloch':
        outputs.push({ name: 'bloch', slot: op.slot, data: [...ref.bloch(state, op.bit, mask, value), op.expectedStatus ?? 0] })
        break
      case 'probability': {
        const raw = ref.probability(state, op.baseBit, op.span, mask, value)
        outputs.push({ name: 'probabilityRaw', slot: op.slot, data: raw })
        outputs.push({ name: 'probability', slot: op.slot, data: ref.normalize(raw) })
        break
      }
      case 'amplitude': {
        const slices = Array.from({ length: state.length >> op.span }, (_, rest) => ({
          rest,
          mag: Array.from({ length: 1 << op.span }, (_, outcome) => {
            const index = ref.insert(rest, outcome, op.baseBit, op.span)
            return (index & mask) === value ? ref.mag(state[index]) : 0
          }).reduce((a, b) => a + b, 0),
        })).sort((a, b) => b.mag - a.mag)
        if (
          slices.length > 1 &&
          slices[0].mag > 1e-12 &&
          (slices[0].mag - slices[1].mag) / slices[0].mag < 1e-4
        )
          throw Error(`case error ${c.name}: ambiguous amplitude slice`)
        if (op.span !== c.qubits) {
          const best = slices[0].rest
          let strongest = 0
          for (let k = 0; k < 1 << op.span; k++) {
            const i = ref.insert(best, k, op.baseBit, op.span),
              p = (i & mask) === value ? ref.mag(state[i]) : 0
            if (strongest && Math.abs(p - strongest * 10000) < Math.max(p, strongest * 10000) * 1e-3)
              throw Error(`case error ${c.name}: ambiguous phase-lock threshold`)
            if (p > strongest * 10000) strongest = p
          }
          if (strongest && Math.abs(strongest - 1e-8) <= 1e-11)
            throw Error(`case error ${c.name}: phase-lock enable threshold`)
        }
        const a = ref.amplitude(state, op.baseBit, op.span, mask, value, Number(op.span !== c.qubits))
        const data = Array(2 * rustConst('MAX_AMPLITUDE_OUTCOMES') + (1 << op.span)).fill(NaN)
        a.ket.forEach(([re, im], i) => {
          data[2 * i] = re
          data[2 * i + 1] = im
        })
        a.incoherent.forEach((v, i) => {
          data[2 * rustConst('MAX_AMPLITUDE_OUTCOMES') + i] = v
        })
        outputs.push({ name: 'amplitude', slot: op.slot, data })
        outputs.push({ name: 'amplitudeMeta', slot: op.slot, data: a.meta, categorical: [1, 2, 3] })
        break
      }
      case 'density': {
        const d = ref.density(state, op.baseBit, op.span, mask, value)
        outputs.push({ name: 'density', slot: op.slot, data: d.data.flat() })
        outputs.push({
          name: 'densityMeta',
          slot: op.slot,
          data: [d.meta, op.span, 0, 0],
          categorical: [1, 2, 3],
        })
        break
      }
    }
  }
  return { state: state.flat(), outputs }
}
function rounded(values: number[]) {
  return values.map((x) => {
    const rounded = Math.round(x * 1e12) / 1e12
    return Object.is(rounded, -0) ? 0 : rounded
  })
}

for (const seed of [1, 2]) {
  test(`Write0 oracle selects global Bell branch for seed ${seed}, never a per-pair merge`, () => {
    const s = Math.SQRT1_2
    const init: ref.C[] = [[s, 0], [0, 0], [0, 0], [0, s]]
    const branch = expected({ name: 'Bell', qubits: 2, init, ops: reset(0, seed) })
    const wanted = seed === 1 ? [1, 0, 0, 0, 0, 0, 0, 0] : [0, 0, 0, 0, 0, 1, 0, 0]
    assert.deepEqual(rounded(branch.state), wanted)
  })

  test(`Write0 oracle keeps surviving complex phases and ratios for seed ${seed}`, () => {
    // Both global branches have probability 1/2; their relative phases differ.
    const init: ref.C[] = [[0.3, 0.4], [0, -0.5], [-0.4, 0.3], [0.5, 0]]
    const state = expected({ name: 'complex', qubits: 2, init, ops: reset(0, seed) }).state
    const branch = seed === 1 ? 0 : 1
    const wanted = [init[branch], [0, 0], init[branch + 2], [0, 0]].flat()
      .map((x) => x * Math.SQRT2)
    assert.deepEqual(rounded(state), rounded(wanted))
  })
}

function controlledReset(seed: number) {
  const init: ref.C[] = [[0.5, 0], [0, 0.5], [-0.5, 0], [0, -0.5]]
  return expected({ name: 'controlled', qubits: 2, init,
    ops: reset(0, seed, 0, { mask: 2, value: 0 }) })
}

test('Write0 controlled oracle measures P=controls-match AND target-one', () => {
  assert.equal(controlledReset(1).outputs[0].data[0], 0.75)
})

test('Write0 controlled no-click oracle projects out only the active target-one branch', () => {
  const amplitude = 1 / Math.sqrt(3)
  assert.deepEqual(rounded(controlledReset(1).state),
    rounded([amplitude, 0, 0, 0, -amplitude, 0, 0, -amplitude]))
})

test('Write0 controlled click oracle retains only the active branch and resets it', () => {
  assert.deepEqual(rounded(controlledReset(2).state), [0, 1, 0, 0, 0, 0, 0, 0])
})

test('Write0 after H is zero', () => {
  const plus = expected({ name: 'H', qubits: 1, init: 'ground',
    ops: [{ kind: 'gate', bit: 0, m: matrices.H }, ...reset(0)] })
  assert.deepEqual(rounded(plus.state), [1, 0, 0, 0])
})

test('Write0 after impossible postselection stays zero', () => {
  const zero = expected({ name: 'zero', qubits: 1, init: [[0, 0], [0, 0]], ops: reset(0) })
  assert.deepEqual(rounded(zero.state), [0, 0, 0, 0])
})

test('Write0 after impossible postselection reports no measurement outcome', () => {
  const zero = expected({ name: 'zero', qubits: 1, init: [[0, 0], [0, 0]], ops: reset(0) })
  assert.equal(zero.outputs[0].data[2], -1)
})

test('real WGSL numeric kernels', async (t) => {
  const { device, assertHealthy } = await openGpu()
  const maxima = Object.fromEntries(kernels.map((key) => [key, 0])) as Record<
    (typeof kernels)[number],
    number
  >
  const counts = Object.fromEntries(kernels.map((key) => [key, 0])) as Record<
    (typeof kernels)[number],
    number
  >
  try {
    const rig = createRig(device)
    const record = (
      key: (typeof kernels)[number],
      name: string,
      output: Parameters<typeof compare>[2],
      values: number[],
      categorical?: number[],
    ) => {
      maxima[key] = Math.max(maxima[key], compare(wgsl(key).path, name, output, values, categorical))
      counts[key]++
    }

    for (const c of cases) {
      await t.test(c.name, async (t) => {
        const cpu = expected(c)
        const gpu = await rig.recompute(c)
        if (c.ops.some((op) => op.kind === 'gate' || op.kind === 'collapse')) {
          const lastWriter = [...c.ops].reverse().find((op) => op.kind === 'gate' || op.kind === 'collapse')!
          const key = lastWriter.kind === 'collapse' ? 'measure_collapse' : 'state_compute'
          const state = {
            name: 'state',
            slot: 0,
            offset: 0,
            expected: gpu.state.length,
            values: gpu.state,
            bits: new Uint32Array(gpu.state.buffer),
            context: gpu.trace,
          }
          await t.test('state', () => record(key, c.name, state, cpu.state))
        }
        if (gpu.outputs.length !== cpu.outputs.length) {
          throw new Error(`Expected outputs length ${cpu.outputs.length}, got ${gpu.outputs.length}`)
        }
        for (const [index, output] of gpu.outputs.entries()) {
          const reference = cpu.outputs[index]
          if (output.name !== reference.name) {
            throw new Error(`Expected output name ${reference.name}, got ${output.name}`)
          }
          const key =
            output.name === 'measurement'
              ? 'measure_reduce'
              : output.name === 'probabilityRaw'
                ? 'probability_reduce'
                : output.name === 'probability'
                  ? 'probability_normalize'
                  : output.name === 'amplitude' || output.name === 'amplitudeMeta'
                    ? 'amplitude_capture'
                    : output.name === 'density' || output.name === 'densityMeta'
                      ? 'density_capture'
                      : output.name === 'bloch'
                        ? 'bloch_reduce'
                        : (output.name as (typeof kernels)[number])
          await t.test(`${output.name} ${index}`, () => {
            record(key, c.name, output, reference.data, reference.categorical)
          })
        }
        assertHealthy()
      })
    }

    const references = aggregateInstances.map((instance) =>
      ref.aggregateExpectedRows(instance, aggregateInput),
    )
    const probabilities = [2, 8, 10, 12, 14].map((slot) => ({ slot, data: aggregateInput }))
    const aggregated = await rig.aggregate(aggregateInstances, probabilities)
    for (const [index, instance] of aggregateInstances.entries()) {
      await t.test(`aggregate slot ${instance.slot}`, () => {
        const rows = references[index]
        record(
          'probability_aggregate',
          `instance ${index}`,
          aggregated[index],
          rows,
          rows.map((_, i) => i),
        )
        assertHealthy()
      })
    }
  } finally {
    for (const key of kernels) console.log(`${key}: ${counts[key]} comparisons, max |error| = ${maxima[key]}`)
    device.destroy()
  }
})
