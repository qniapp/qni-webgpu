import { test } from 'node:test'
import assert from 'node:assert/strict'
import { openGpu } from './gpu'
import { createRig } from './rig'
import type { Op } from './cases'

for (const kind of ['bloch', 'probability', 'amplitude', 'density'] as const) {
  for (const emptySlice of [false, true]) {
    test(`${kind} marks ${emptySlice ? 'empty control slice' : 'zero state'} undefined`, async () => {
      const { device, assertHealthy } = await openGpu()
      try {
        const controls = emptySlice ? { mask: 2, value: 2 } : undefined
        const op: Op = kind === 'bloch'
          ? { kind, bit: 0, slot: 0, controls }
          : { kind, baseBit: 0, span: 1, slot: 0, controls }
        const result = await createRig(device).recompute({
          name: 'undefined display', qubits: 2,
          init: emptySlice ? 'ground' : [[0, 0], [0, 0], [0, 0], [0, 0]], ops: [op],
        })
        const name = kind === 'amplitude' ? 'amplitudeMeta' : kind === 'density' ? 'densityMeta' : kind
        const values = [...result.outputs.find((output) => output.name === name)!.values]
        assertHealthy()
        assert.deepEqual(values, kind === 'bloch' ? [0, 0, 0, -1]
          : kind === 'probability' ? [-1, -1]
          : kind === 'amplitude' ? [0, -1, 1, -1] : [0, 1, 0, 0])
      } finally {
        device.destroy()
      }
    })
  }
}
