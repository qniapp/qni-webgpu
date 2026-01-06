import type { SceneLayout } from '../ui/layout'
import type { PlacedGate, ShapeInstance, TextLayout } from '../ui/types'
import { COLORS, GATE_SIZE, STATE_CARD_LINE_OFFSETS } from '../ui/constants'
import { FONT_GLYPH_SIZE, LABEL_GLYPH_SIZE } from '../ui/text'
import { shapeShaderCode, textShaderCode } from '../gpu/shaders'

type TextBuffers = {
  textBindGroup: GPUBindGroup
  glyphCount: number
  uniformBuffer: GPUBuffer
  glyphBuffer: GPUBuffer
  label?: string
}

type RendererOptions = {
  device: GPUDevice
  context: GPUCanvasContext
  format: GPUTextureFormat
  canvasWidth: number
  canvasHeight: number
  fontTexture: GPUTexture
  labelFontTexture: GPUTexture
  fontAtlasWidth: number
  fontAtlasHeight: number
  labelAtlasWidth: number
  labelAtlasHeight: number
  fontSampler: GPUSampler
  labelSampler: GPUSampler
  stateTextGlyphBuffer: GPUBuffer
}

export function createRenderer(options: RendererOptions) {
  const {
    device,
    context,
    format,
    canvasWidth: initialCanvasWidth,
    canvasHeight: initialCanvasHeight,
    fontTexture,
    labelFontTexture,
    fontAtlasWidth,
    fontAtlasHeight,
    labelAtlasWidth,
    labelAtlasHeight,
    fontSampler,
    labelSampler,
    stateTextGlyphBuffer,
  } = options

  const instanceStride = 11
  let instanceCapacity = 8
  let instanceData = new Float32Array(instanceCapacity * instanceStride)
  let instanceCount = 0
  let instanceBuffer = device.createBuffer({
    size: instanceData.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  })

  let canvasWidth = initialCanvasWidth
  let canvasHeight = initialCanvasHeight

  const uniformBuffer = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  })
  device.queue.writeBuffer(uniformBuffer, 0, new Float32Array([canvasWidth, canvasHeight, 0, 0]))

  const shapeModule = device.createShaderModule({ code: shapeShaderCode })
  const shapePipeline = device.createRenderPipeline({
    layout: 'auto',
    vertex: {
      module: shapeModule,
      entryPoint: 'vs_main',
      buffers: [
        {
          arrayStride: instanceStride * 4,
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32' },
            { shaderLocation: 1, offset: 1 * 4, format: 'float32' },
            { shaderLocation: 2, offset: 2 * 4, format: 'float32x2' },
            { shaderLocation: 3, offset: 4 * 4, format: 'float32x2' },
            { shaderLocation: 4, offset: 6 * 4, format: 'float32x4' },
          ],
        },
      ],
    },
    fragment: {
      module: shapeModule,
      entryPoint: 'fs_main',
      targets: [
        {
          format,
          blend: {
            color: {
              srcFactor: 'src-alpha',
              dstFactor: 'one-minus-src-alpha',
              operation: 'add',
            },
            alpha: {
              srcFactor: 'one',
              dstFactor: 'one-minus-src-alpha',
              operation: 'add',
            },
          },
        },
      ],
    },
    primitive: { topology: 'triangle-list' },
  })

  const shapeBindGroup = device.createBindGroup({
    layout: shapePipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: uniformBuffer } }],
  })

  const textModule = device.createShaderModule({ code: textShaderCode })
  const textPipeline = device.createRenderPipeline({
    layout: 'auto',
    vertex: { module: textModule, entryPoint: 'vs_main' },
    fragment: {
      module: textModule,
      entryPoint: 'fs_main',
      targets: [
        {
          format,
          blend: {
            color: {
              srcFactor: 'src-alpha',
              dstFactor: 'one-minus-src-alpha',
              operation: 'add',
            },
            alpha: {
              srcFactor: 'one',
              dstFactor: 'one-minus-src-alpha',
              operation: 'add',
            },
          },
        },
      ],
    },
    primitive: { topology: 'triangle-list' },
  })

  const gateOverlayBuffer = device.createBuffer({
    size: instanceStride * 4,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  })
  let hasGateOverlay = false

  const updateGateOverlay = (draggingGate: PlacedGate | null) => {
    if (!draggingGate) {
      hasGateOverlay = false
      return
    }
    const gateColor = COLORS.boxActive
    const overlayData = new Float32Array([
      2,
      6,
      draggingGate.x,
      draggingGate.y,
      GATE_SIZE,
      GATE_SIZE,
      gateColor[0],
      gateColor[1],
      gateColor[2],
      gateColor[3],
      0,
    ])
    device.queue.writeBuffer(gateOverlayBuffer, 0, overlayData)
    hasGateOverlay = true
  }

  const updateTextUniform = (
    buffer: GPUBuffer,
    layout: TextLayout,
    glyphSize: number,
    atlasWidth: number,
    atlasHeight: number,
    glyphOffset = 0
  ) => {
    const uniformData = new Float32Array([
      canvasWidth,
      canvasHeight,
      layout.x,
      layout.y,
      glyphSize,
      glyphSize,
      atlasWidth,
      atlasHeight,
      layout.color[0],
      layout.color[1],
      layout.color[2],
      layout.color[3],
      glyphOffset,
      0,
      0,
      0,
    ])
    device.queue.writeBuffer(buffer, 0, uniformData)
  }

  const setSize = (width: number, height: number) => {
    canvasWidth = width
    canvasHeight = height
    device.queue.writeBuffer(uniformBuffer, 0, new Float32Array([canvasWidth, canvasHeight, 0, 0]))
  }

  const makeTextBuffers = (
    layout: TextLayout,
    options?: {
      glyphBuffer?: GPUBuffer
      glyphCount?: number
      glyphSize?: number
      atlasWidth?: number
      atlasHeight?: number
      texture?: GPUTexture
      sampler?: GPUSampler
      glyphOffset?: number
    }
  ) => {
    let glyphBuffer = options?.glyphBuffer
    let glyphCount = options?.glyphCount ?? layout.text.length
    if (!glyphBuffer) {
      const codes = new Uint32Array(Array.from(layout.text).map((char) => char.charCodeAt(0)))
      glyphBuffer = device.createBuffer({
        size: codes.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      })
      device.queue.writeBuffer(glyphBuffer, 0, codes)
      glyphCount = codes.length
    }

    const glyphSize = options?.glyphSize ?? FONT_GLYPH_SIZE
    const atlasWidth = options?.atlasWidth ?? fontAtlasWidth
    const atlasHeight = options?.atlasHeight ?? fontAtlasHeight
    const uniformBuffer = device.createBuffer({
      size: 64,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })
    updateTextUniform(uniformBuffer, layout, glyphSize, atlasWidth, atlasHeight, options?.glyphOffset ?? 0)

    const textBindGroup = device.createBindGroup({
      layout: textPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: uniformBuffer } },
        { binding: 1, resource: { buffer: glyphBuffer } },
        { binding: 2, resource: options?.sampler ?? fontSampler },
        { binding: 3, resource: (options?.texture ?? fontTexture).createView() },
      ],
    })

    return { textBindGroup, glyphCount, uniformBuffer, glyphBuffer }
  }

  let gateTexts: TextBuffers[] = []
  let paletteTexts: TextBuffers[] = []
  let wireTexts: TextBuffers[] = []
  let stateTextDraws: TextBuffers[] = []

  const syncGateTexts = (layouts: TextLayout[]) => {
    if (gateTexts.length !== layouts.length) {
      gateTexts = layouts.map((layout) => ({
        ...makeTextBuffers(layout, {
          glyphSize: LABEL_GLYPH_SIZE,
          atlasWidth: labelAtlasWidth,
          atlasHeight: labelAtlasHeight,
          texture: labelFontTexture,
          sampler: labelSampler,
        }),
        label: layout.text,
      }))
      return
    }
    layouts.forEach((layout, index) => {
      const gateText = gateTexts[index]
      if (gateText.label !== layout.text) {
        const codes = new Uint32Array(Array.from(layout.text).map((char) => char.charCodeAt(0)))
        device.queue.writeBuffer(gateText.glyphBuffer, 0, codes)
        gateText.label = layout.text
      }
      updateTextUniform(gateText.uniformBuffer, layout, LABEL_GLYPH_SIZE, labelAtlasWidth, labelAtlasHeight)
      gateText.glyphCount = layout.text.length
    })
  }

  const updateInstanceBuffer = (instances: ShapeInstance[]) => {
    if (instances.length > instanceCapacity) {
      instanceCapacity = instances.length
      instanceData = new Float32Array(instanceCapacity * instanceStride)
      instanceBuffer = device.createBuffer({
        size: instanceData.byteLength,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      })
    } else {
      instanceData.fill(0)
    }
    instances.forEach((instance, index) => {
      const offset = index * instanceStride
      instanceData[offset] = instance.kind
      instanceData[offset + 1] = instance.thickness
      instanceData[offset + 2] = instance.p0x
      instanceData[offset + 3] = instance.p0y
      instanceData[offset + 4] = instance.p1x
      instanceData[offset + 5] = instance.p1y
      instanceData[offset + 6] = instance.color[0]
      instanceData[offset + 7] = instance.color[1]
      instanceData[offset + 8] = instance.color[2]
      instanceData[offset + 9] = instance.color[3]
      instanceData[offset + 10] = 0
    })
    instanceCount = instances.length
    device.queue.writeBuffer(instanceBuffer, 0, instanceData)
  }

  const updateScene = (scene: SceneLayout, draggingGate: PlacedGate | null) => {
    updateInstanceBuffer(scene.instances)
    syncGateTexts(scene.gateLabels)

    if (paletteTexts.length === 0) {
      paletteTexts = scene.paletteLabels.map((layout) =>
        makeTextBuffers(layout, {
          glyphSize: LABEL_GLYPH_SIZE,
          atlasWidth: labelAtlasWidth,
          atlasHeight: labelAtlasHeight,
          texture: labelFontTexture,
          sampler: labelSampler,
        })
      )
    } else {
      scene.paletteLabels.forEach((layout, index) => {
        if (!paletteTexts[index]) {
          paletteTexts.push(
            makeTextBuffers(layout, {
              glyphSize: LABEL_GLYPH_SIZE,
              atlasWidth: labelAtlasWidth,
              atlasHeight: labelAtlasHeight,
              texture: labelFontTexture,
              sampler: labelSampler,
            })
          )
          return
        }
        updateTextUniform(paletteTexts[index].uniformBuffer, layout, LABEL_GLYPH_SIZE, labelAtlasWidth, labelAtlasHeight)
      })
    }

    if (wireTexts.length === 0) {
      wireTexts = scene.wireLabels.map((layout) => makeTextBuffers(layout))
    } else {
      scene.wireLabels.forEach((layout, index) => {
        if (!wireTexts[index]) {
          wireTexts.push(makeTextBuffers(layout))
          return
        }
        updateTextUniform(wireTexts[index].uniformBuffer, layout, FONT_GLYPH_SIZE, fontAtlasWidth, fontAtlasHeight)
      })
    }

    if (stateTextDraws.length === 0) {
      stateTextDraws = scene.stateVectorLines.map((layout, index) =>
        makeTextBuffers(layout, {
          glyphBuffer: stateTextGlyphBuffer,
          glyphCount: layout.glyphCount ?? 0,
          glyphOffset: STATE_CARD_LINE_OFFSETS[index] ?? 0,
        })
      )
    } else {
      scene.stateVectorLines.forEach((layout, index) => {
        if (!stateTextDraws[index]) {
          stateTextDraws.push(
            makeTextBuffers(layout, {
              glyphBuffer: stateTextGlyphBuffer,
              glyphCount: layout.glyphCount ?? 0,
              glyphOffset: STATE_CARD_LINE_OFFSETS[index] ?? 0,
            })
          )
          return
        }
        updateTextUniform(
          stateTextDraws[index].uniformBuffer,
          layout,
          FONT_GLYPH_SIZE,
          fontAtlasWidth,
          fontAtlasHeight,
          STATE_CARD_LINE_OFFSETS[index] ?? 0
        )
        stateTextDraws[index].glyphCount = layout.glyphCount ?? 0
      })
    }

    updateGateOverlay(draggingGate)
  }

  const renderFrame = () => {
    const commandEncoder = device.createCommandEncoder()
    const currentTexture = context.getCurrentTexture()
    const pass = commandEncoder.beginRenderPass({
      colorAttachments: [
        {
          view: currentTexture.createView(),
          clearValue: { r: COLORS.background[0], g: COLORS.background[1], b: COLORS.background[2], a: 1 },
          loadOp: 'clear',
          storeOp: 'store',
        },
      ],
    })

    pass.setPipeline(shapePipeline)
    pass.setBindGroup(0, shapeBindGroup)
    pass.setVertexBuffer(0, instanceBuffer)
    pass.draw(6, instanceCount, 0, 0)

    pass.setPipeline(textPipeline)
    for (const paletteText of paletteTexts) {
      pass.setBindGroup(0, paletteText.textBindGroup)
      pass.draw(6, paletteText.glyphCount, 0, 0)
    }

    for (const wireText of wireTexts) {
      pass.setBindGroup(0, wireText.textBindGroup)
      pass.draw(6, wireText.glyphCount, 0, 0)
    }

    if (hasGateOverlay) {
      pass.setPipeline(shapePipeline)
      pass.setBindGroup(0, shapeBindGroup)
      pass.setVertexBuffer(0, gateOverlayBuffer)
      pass.draw(6, 1, 0, 0)
      pass.setPipeline(textPipeline)
    }

    for (const gateText of gateTexts) {
      pass.setBindGroup(0, gateText.textBindGroup)
      pass.draw(6, gateText.glyphCount, 0, 0)
    }

    for (const stateTextDraw of stateTextDraws) {
      pass.setBindGroup(0, stateTextDraw.textBindGroup)
      pass.draw(6, stateTextDraw.glyphCount, 0, 0)
    }

    pass.end()
    device.queue.submit([commandEncoder.finish()])
  }

  return { renderFrame, updateScene, setSize }
}
