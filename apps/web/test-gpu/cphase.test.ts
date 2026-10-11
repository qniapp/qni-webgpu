// Same-column P gates with one angle form a multi-controlled phase (issue #57).
// `linearize_ops` emits each group as one phase gate on the top wire controlled
// by the other members (plus the column's • / ◦); these cases run that op shape
// on the real WGSL kernels and compare against closed-form CPHASE amplitudes,
// not against a replay of the same ops.
import { test } from 'node:test'
import { openGpu } from './gpu'
import { createRig } from './rig'
import { gate, rotation, type Op, type RecomputeCase } from './cases'
import type { C } from './reference'
import { compare } from './check'
import { wgsl } from './app-source'

// e^{iφ} applies to basis index i when (i & mask) === value.
type Phase = { angle: number; mask: number; value: number }

const uniform = (qubits: number): C[] =>
  Array.from({ length: 1 << qubits }, () => [Math.fround(1 / Math.sqrt(1 << qubits)), 0])

function closedForm(init: C[], phases: Phase[]): number[] {
  return init.flatMap(([re, im], i) => {
    const angle = phases.reduce((sum, p) => sum + ((i & p.mask) === p.value ? p.angle : 0), 0)
    const [c, s] = [Math.cos(angle), Math.sin(angle)]
    return [re * c - im * s, re * s + im * c]
  })
}

const phaseOp = (bit: number, angle: number, mask = 0, value = 0): Op =>
  gate(bit, rotation('phase', angle), mask ? { mask, value } : undefined)

type Case = RecomputeCase & { init: C[]; phases: Phase[] }

const cases: Case[] = [
  {
    // Tutorial cphase.html: P(π/4) on bits 0 and 2 of |+++>.
    name: 'CPHASE P(π/4) · P(π/4)',
    qubits: 3,
    init: uniform(3),
    ops: [phaseOp(0, Math.PI / 4, 0b100, 0b100)],
    phases: [{ angle: Math.PI / 4, mask: 0b101, value: 0b101 }],
  },
  {
    name: 'CCPHASE P(π/4) × 3',
    qubits: 3,
    init: uniform(3),
    ops: [phaseOp(0, Math.PI / 4, 0b110, 0b110)],
    phases: [{ angle: Math.PI / 4, mask: 0b111, value: 0b111 }],
  },
  {
    name: 'CPHASE with • joins the condition',
    qubits: 4,
    init: uniform(4),
    ops: [phaseOp(0, Math.PI / 3, 0b1010, 0b1010)],
    phases: [{ angle: Math.PI / 3, mask: 0b1011, value: 0b1011 }],
  },
  {
    name: 'CPHASE with ◦ requires |0>',
    qubits: 3,
    init: uniform(3),
    ops: [phaseOp(0, Math.PI / 4, 0b110, 0b100)],
    phases: [{ angle: Math.PI / 4, mask: 0b111, value: 0b101 }],
  },
  {
    name: 'mixed angles: CPHASE(π/4) on bits 0,2 and lone P(π/2) on bit 1',
    qubits: 3,
    init: uniform(3),
    ops: [phaseOp(0, Math.PI / 4, 0b100, 0b100), phaseOp(1, Math.PI / 2)],
    phases: [
      { angle: Math.PI / 4, mask: 0b101, value: 0b101 },
      { angle: Math.PI / 2, mask: 0b010, value: 0b010 },
    ],
  },
  ...[
    { outcome: 1, name: 'conditional CPHASE applies after measuring 1' },
    { outcome: 0, name: 'conditional CPHASE is skipped after measuring 0' },
  ].map(({ outcome, name }) => {
    // Bit 0 holds a definite `outcome`; bits 1, 2 are |+>.
    const init: C[] = Array.from({ length: 8 }, (_, i) =>
      (i & 1) === outcome ? [Math.fround(0.5), 0] : [0, 0],
    )
    return {
      name,
      qubits: 3,
      init,
      ops: [
        { kind: 'measure' as const, bit: 0, gateId: 7, slot: 2 },
        { kind: 'collapse' as const, bit: 0, auxSlot: 2 },
        { ...phaseOp(1, Math.PI / 4, 0b100, 0b100), condition: 2 },
      ],
      phases: outcome ? [{ angle: Math.PI / 4, mask: 0b110, value: 0b110 }] : [],
    }
  }),
]

test('same-angle P gates as one multi-controlled phase', async (t) => {
  const { device, assertHealthy } = await openGpu()
  try {
    const rig = createRig(device)
    for (const c of cases) {
      await t.test(c.name, async () => {
        const gpu = await rig.recompute(c)
        const state = {
          name: 'state',
          slot: 0,
          offset: 0,
          expected: gpu.state.length,
          values: gpu.state,
          bits: new Uint32Array(gpu.state.buffer),
          context: gpu.trace,
        }
        compare(wgsl('state_compute').path, c.name, state, closedForm(c.init, c.phases))
        assertHealthy()
      })
    }
  } finally {
    device.destroy()
  }
})
