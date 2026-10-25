import { readFileSync, writeFileSync } from 'node:fs'
import { globals } from 'webgpu'
import { createHash } from 'node:crypto'
import { release, platform, arch } from 'node:os'
import { performance } from 'node:perf_hooks'
import { arithmeticShaderPath } from '../scripts/arithmetic-shader-path'
import { pack } from './app-source'
import { openGpu, scoped } from './gpu'

const { GPUBufferUsage } = globals as typeof globalThis

const batchSize = 256, sampleCount = 24, warmupCount = 20
const median = (xs: number[]) => {
  const sorted = [...xs].sort((a, b) => a - b), mid = sorted.length / 2
  return (sorted[Math.floor(mid)] + sorted[Math.ceil(mid) - 1]) / 2
}
function summarize(ms: number[]) {
  const medianMs = median(ms)
  return { medianMs, madMs: median(ms.map((x) => Math.abs(x - medianMs))),
    minMs: Math.min(...ms), maxMs: Math.max(...ms), samplesMs: ms }
}
async function main() {
  const options: Record<string, string> = {}
  for (const arg of process.argv.slice(2)) {
    const match = arg.match(/^--(shader|baseline|output)=(.+)$/)
    if (!match || options[match[1]]) throw Error('Usage: arithmetic.bench.ts [--shader=PATH] [--baseline=PATH] --output=PATH')
    options[match[1]] = match[2]
  }
  if (!options.output) throw Error('--output=PATH is required')
  // Check explicit inputs before invoking Cargo or opening a device.
  const load = (path: string) => {
    const code = readFileSync(path, 'utf8')
    if (!code.trim()) throw Error(`Empty shader: ${path}`)
    return { path, code, sha256: createHash('sha256').update(code).digest('hex') }
  }
  const baseline = options.baseline ? load(options.baseline) : undefined
  const candidate = load(options.shader ?? arithmeticShaderPath())
  const shaders = baseline ? [candidate, baseline] : [candidate]
  const { device, assertHealthy, adapterInfo, selector } = await openGpu()
  try {
    const pipelines = await scoped(device, () => Promise.all(shaders.map((shader) =>
      device.createComputePipelineAsync({ layout: 'auto', compute: {
        module: device.createShaderModule({ code: shader.code }), entryPoint: 'main',
      } }))))
    const results = []
    for (const qubits of [12, 16]) for (const width of [1, 3, 8, 16]) {
      if (width > qubits) continue
      for (const amount of [1, -1]) {
        const count = 2 ** qubits, shift = width === 16 ? 0 : 1
        const controlMask = width === 16 ? 0 : 1 | (1 << (qubits - 1))
        const params = { state_count: count, target_shift: shift, target_mask: (1 << width) - 1,
          amount: amount >>> 0, control_mask: controlMask, control_value: controlMask ? 1 : 0 }
        const buffers: GPUBuffer[] = []
        try {
          const commands = await scoped(device, async () => pipelines.map((pipeline) => {
            const buffer = (size: number, usage: number) => {
              const b = device.createBuffer({ size, usage }); buffers.push(b); return b
            }
            const states = [0, 1].map(() => buffer(count * 8, GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST))
            // Seed a selected basis state; all remaining bytes are WebGPU-zero-initialized.
            device.queue.writeBuffer(states[0], params.control_value * 8, new Float32Array([1, 0]))
            const data = pack('ArithmeticParams', params)
            const uniform = buffer(data.byteLength, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST)
            device.queue.writeBuffer(uniform, 0, data)
            const groups = [0, 1].map((i) => device.createBindGroup({ layout: pipeline.getBindGroupLayout(0),
              entries: [states[i], states[1 - i], uniform].map((b, binding) => ({ binding, resource: { buffer: b } })) }))
            // Command buffers are single-use: pre-encode every warmup and measured batch.
            return Array.from({ length: warmupCount + sampleCount }, () => {
              const encoder = device.createCommandEncoder(), pass = encoder.beginComputePass()
              pass.setPipeline(pipeline)
              for (let i = 0; i < batchSize; i++) {
                pass.setBindGroup(0, groups[i % 2]); pass.dispatchWorkgroups(Math.ceil(count / 64))
              }
              pass.end()
              return encoder.finish()
            })
          }))
          const next = commands.map(() => 0)
          const submit = async (index: number) => {
            const batch = [commands[index][next[index]++]]
            const start = performance.now()
            device.queue.submit(batch)
            await device.queue.onSubmittedWorkDone()
            const ms = performance.now() - start
            assertHealthy()
            return ms
          }
          // No compilation, allocation, encoding, or readback inside the timed region.
          for (let i = 0; i < warmupCount; i++) {
            const order = commands.length === 2 && i % 2 ? [1, 0] : commands.map((_, j) => j)
            for (const j of order) await submit(j)
          }
          const samples = commands.map(() => [] as number[])
          await scoped(device, async () => {
            for (let i = 0; i < sampleCount; i++) {
              const order = commands.length === 2 && i % 2 ? [1, 0] : commands.map((_, j) => j)
              for (const j of order) samples[j].push(await submit(j))
            }
          })
          const stats = samples.map(summarize)
          const ratio = baseline ? stats[0].medianMs / stats[1].medianMs : null
          const dispersion = baseline ? stats[0].madMs + stats[1].madMs : 0
          const verdict = !baseline ? 'unpaired' : ratio! > 1.05
            ? (stats[0].medianMs - stats[1].medianMs > dispersion ? 'regression' : 'inconclusive')
            : 'no-regression-observed'
          results.push({ qubits, width, amount, params, candidate: stats[0], baseline: stats[1], ratio, verdict })
          console.log(`qubits=${qubits} width=${width} amount=${amount}: candidate=${stats[0].medianMs.toFixed(3)}ms MAD=${stats[0].madMs.toFixed(3)}ms` +
            (baseline ? ` baseline=${stats[1].medianMs.toFixed(3)}ms MAD=${stats[1].madMs.toFixed(3)}ms ratio=${ratio!.toFixed(4)} ${verdict}` : ' (unpaired)'))
        } finally { for (const buffer of buffers) buffer.destroy() }
      }
    }
    const info = adapterInfo
    writeFileSync(options.output, JSON.stringify({
      measurement: 'host submit-to-queue.onSubmittedWorkDone batch latency (includes submission/completion overhead)',
      timestamp: new Date().toISOString(), batchSize, sampleCount, warmupCount, dispersion: 'median absolute deviation (MAD)',
      adapter: { vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description },
      driverVersion: 'not exposed by WebGPU; see adapter description',
      runtime: { node: process.version, versions: process.versions, webgpu: require('webgpu/package.json').version,
        os: `${platform()} ${release()} ${arch()}`, selector },
      shaders: shaders.map(({ path, sha256 }) => ({ path, sha256 })), results,
    }, null, 2) + '\n')
    if (results.some((r) => r.verdict === 'regression')) throw Error('Median regression >5% outside measured MAD; repeat to confirm (results saved)')
    if (results.some((r) => r.verdict === 'inconclusive')) console.error('Inconclusive: timing dispersion masks a >5% difference; repeat measurement')
  } finally { device.destroy() }
}
main().catch((error) => { console.error(String(error)); process.exitCode = 1 })
