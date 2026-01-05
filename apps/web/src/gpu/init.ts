export async function initGpu(canvas: HTMLCanvasElement) {
  const adapter = await navigator.gpu.requestAdapter()
  if (!adapter) {
    throw new Error('No WebGPU adapter')
  }
  const device = await adapter.requestDevice()
  const context = canvas.getContext('webgpu')
  if (!context) {
    throw new Error('WebGPU context unavailable')
  }
  const format = navigator.gpu.getPreferredCanvasFormat()
  context.configure({
    device,
    format,
    alphaMode: 'opaque',
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  })

  return { device, context, format }
}
