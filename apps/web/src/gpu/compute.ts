import { gateToIndex, type Gate } from '../domain/gate'
import { STATE_TEXT_MAX_LEN } from '../ui/constants'
import { computeShaderCode, stateTextComputeCode } from './shaders'

export async function computeStateVector(
  device: GPUDevice,
  gate: Gate,
  readback: boolean
): Promise<{ outputBuffer: GPUBuffer; readback: Float32Array | null }> {
  const inputState = new Float32Array([1, 0, 0, 0])
  const inputBuffer = device.createBuffer({
    size: inputState.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  })
  device.queue.writeBuffer(inputBuffer, 0, inputState)

  const outputBuffer = device.createBuffer({
    size: inputState.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  })

  const paramsBuffer = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  })
  device.queue.writeBuffer(paramsBuffer, 0, new Uint32Array([gateToIndex(gate), 0, 0, 0]))

  const shaderModule = device.createShaderModule({ code: computeShaderCode })
  const compilationInfo = await shaderModule.getCompilationInfo()
  const compilationErrors = compilationInfo.messages.filter((message) => message.type === 'error')
  if (compilationErrors.length > 0) {
    const message = compilationErrors.map((error) => error.message).join(' | ')
    throw new Error(message)
  }

  const pipeline = device.createComputePipeline({
    layout: 'auto',
    compute: {
      module: shaderModule,
      entryPoint: 'cs_main',
    },
  })

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: paramsBuffer } },
      { binding: 1, resource: { buffer: inputBuffer } },
      { binding: 2, resource: { buffer: outputBuffer } },
    ],
  })

  const commandEncoder = device.createCommandEncoder()
  const pass = commandEncoder.beginComputePass()
  pass.setPipeline(pipeline)
  pass.setBindGroup(0, bindGroup)
  pass.dispatchWorkgroups(1)
  pass.end()
  if (readback) {
    const readbackBuffer = device.createBuffer({
      size: inputState.byteLength,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    })
    commandEncoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, inputState.byteLength)
    device.queue.submit([commandEncoder.finish()])
    await readbackBuffer.mapAsync(GPUMapMode.READ)
    const mapped = new Float32Array(readbackBuffer.getMappedRange())
    const result = new Float32Array(mapped)
    readbackBuffer.unmap()
    return { outputBuffer, readback: result }
  }
  device.queue.submit([commandEncoder.finish()])
  return { outputBuffer, readback: null }
}

export async function populateStateTextBuffer(
  device: GPUDevice,
  stateVectorBuffer: GPUBuffer,
  glyphBuffer: GPUBuffer
): Promise<void> {
  const shaderModule = device.createShaderModule({ code: stateTextComputeCode })
  const compilationInfo = await shaderModule.getCompilationInfo()
  const compilationErrors = compilationInfo.messages.filter((message) => message.type === 'error')
  if (compilationErrors.length > 0) {
    const message = compilationErrors.map((error) => error.message).join(' | ')
    throw new Error(message)
  }

  const pipeline = device.createComputePipeline({
    layout: 'auto',
    compute: {
      module: shaderModule,
      entryPoint: 'cs_main',
    },
  })

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: stateVectorBuffer } },
      { binding: 1, resource: { buffer: glyphBuffer } },
    ],
  })

  const commandEncoder = device.createCommandEncoder()
  const pass = commandEncoder.beginComputePass()
  pass.setPipeline(pipeline)
  pass.setBindGroup(0, bindGroup)
  pass.dispatchWorkgroups(Math.ceil(STATE_TEXT_MAX_LEN / 64))
  pass.end()
  device.queue.submit([commandEncoder.finish()])
}
