import { test } from 'node:test'
import { globals } from 'webgpu'
import assert from 'node:assert/strict'
import { openGpu } from './gpu'
import { checkDrift, pack, wgsl } from './app-source'

const { GPUBufferUsage, GPUMapMode } = globals as typeof globalThis

// Test-only reference scatters input amplitudes forward; the shader gathers
// output amplitudes through the inverse map, so these are independent paths.
for (const span of [1, 2, 3, 4, 16]) {
  for (const amount of [1, -1]) {
    for (const controlled of span === 16 ? [false] : [false, true]) {
      test(`arithmetic span=${span} amount=${amount} controlled=${controlled}`, async () => {
        checkDrift()
        const { device, assertHealthy } = await openGpu()
        try {
          const shift = controlled ? 1 : 0
          const count = 1 << (span + (controlled ? 3 : 0))
          const mask = (1 << span) - 1
          const controlMask = controlled ? (1 | (1 << (span + 1)) | (1 << (span + 2))) : 0
          const controlValue = controlled ? (1 | (1 << (span + 1))) : 0
          const input = Float32Array.from({ length: count * 2 }, (_, i) => Math.sin(i * 0.71) / Math.sqrt(count))
          const expected = new Float32Array(input.length)
          for (let i = 0; i < count; i++) {
            const field = (i >> shift) & mask
            const target = (i & controlMask) === controlValue
              ? (i & ~(mask << shift)) | (((field + amount) & mask) << shift) : i
            expected[2 * target] = input[2 * i]
            expected[2 * target + 1] = input[2 * i + 1]
          }
          const buffer = (size: number, usage: number) => device.createBuffer({ size, usage })
          const src = buffer(input.byteLength, GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST)
          const dst = buffer(input.byteLength, GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC)
          const uniformData = pack('ArithmeticParams', { state_count: count, target_shift: shift, target_mask: mask,
            amount: amount >>> 0, control_mask: controlMask, control_value: controlValue })
          const uniform = buffer(uniformData.byteLength, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST)
          const readback = buffer(input.byteLength, GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST)
          device.queue.writeBuffer(src, 0, input)
          device.queue.writeBuffer(uniform, 0, uniformData)
          const pipeline = device.createComputePipeline({ layout: 'auto', compute: {
            module: device.createShaderModule({ code: wgsl('arithmetic').code }), entryPoint: 'main' } })
          const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries:
            [src, dst, uniform].map((b, binding) => ({ binding, resource: { buffer: b } })) })
          const encoder = device.createCommandEncoder()
          const pass = encoder.beginComputePass()
          pass.setPipeline(pipeline)
          pass.setBindGroup(0, group)
          pass.dispatchWorkgroups(Math.ceil(count / 64))
          pass.end()
          encoder.copyBufferToBuffer(dst, 0, readback, 0, input.byteLength)
          device.queue.submit([encoder.finish()])
          await readback.mapAsync(GPUMapMode.READ)
          const actual = new Float32Array(readback.getMappedRange()).slice()
          readback.unmap()
          assertHealthy()
          assert.deepEqual(actual, expected)
        } finally { device.destroy() }
      })
    }
  }
}
