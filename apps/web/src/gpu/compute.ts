import { gateToIndex, type Gate } from '../domain/gate'
import { computeShaderCode, stateTextComputeCode } from './shaders'

export type GateOperation = {
  gate: Gate
  target: number
}

export async function computeStateVector(
  device: GPUDevice,
  gate: Gate,
  readback: boolean
): Promise<{ outputBuffer: GPUBuffer; readback: Float32Array | null }> {
  return computeStateVectorSequence(device, [{ gate, target: 0 }], readback)
}

export async function computeStateVectorSequence(
  device: GPUDevice,
  gates: GateOperation[],
  readback: boolean
): Promise<{ outputBuffer: GPUBuffer; readback: Float32Array | null }> {
  const inputState = new Float32Array([1, 0, 0, 0, 0, 0, 0, 0])
  const bufferA = device.createBuffer({
    size: inputState.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
  })
  device.queue.writeBuffer(bufferA, 0, inputState)

  const bufferB = device.createBuffer({
    size: inputState.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
  })

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

  let inputBuffer = bufferA
  let outputBuffer = bufferB

  if (gates.length === 0) {
    outputBuffer = inputBuffer
  } else {
    gates.forEach((operation) => {
      const paramsBuffer = device.createBuffer({
        size: 16,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      })
      device.queue.writeBuffer(paramsBuffer, 0, new Uint32Array([gateToIndex(operation.gate), operation.target, 0, 0]))
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
      device.queue.submit([commandEncoder.finish()])
      const swap = inputBuffer
      inputBuffer = outputBuffer
      outputBuffer = swap
    })
    outputBuffer = inputBuffer
  }

  if (readback) {
    const readbackBuffer = device.createBuffer({
      size: inputState.byteLength,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    })
    const commandEncoder = device.createCommandEncoder()
    commandEncoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, inputState.byteLength)
    device.queue.submit([commandEncoder.finish()])
    await readbackBuffer.mapAsync(GPUMapMode.READ)
    const mapped = new Float32Array(readbackBuffer.getMappedRange())
    const result = new Float32Array(mapped)
    readbackBuffer.unmap()
    return { outputBuffer, readback: result }
  }
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
  pass.dispatchWorkgroups(1)
  pass.end()
  device.queue.submit([commandEncoder.finish()])
}
