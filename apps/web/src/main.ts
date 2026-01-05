import './style.css'
import { computeStateVectorSequence, populateStateTextBuffer } from './gpu/compute'
import { initGpu } from './gpu/init'
import { createRenderer } from './renderer/renderer'
import { CANVAS_HEIGHT, CANVAS_WIDTH, STATE_TEXT_MAX_LEN } from './ui/constants'
import { setupInput } from './ui/input'
import { buildScene } from './ui/layout'
import { BASE_GLYPHS, FONT_GLYPH_SIZE, LABEL_GLYPH_SIZE, buildLabelGlyphs, createFontAtlas } from './ui/text'
import type { PlacedGate } from './ui/types'

declare global {
  interface Window {
    __renderDone?: boolean
    __vertexCount?: number
    __debugPixel?: number[]
    __captureFrame?: boolean
    __captureStateVector?: boolean
    __frameDataUrl?: string
    __stateVector?: number[]
  }
}

const app = document.querySelector<HTMLDivElement>('#app')
if (!app) {
  throw new Error('#app not found')
}

app.innerHTML = `
  <canvas id="gfx" width="${CANVAS_WIDTH}" height="${CANVAS_HEIGHT}"></canvas>
  <div id="status" aria-live="polite"></div>
`

const statusEl = document.querySelector<HTMLDivElement>('#status')
const canvas = document.querySelector<HTMLCanvasElement>('#gfx')
if (!canvas) {
  throw new Error('#gfx not found')
}

function setStatus(message: string) {
  if (statusEl) {
    statusEl.textContent = message
  }
}

if (!navigator.gpu) {
  setStatus('WebGPU is not supported in this browser.')
  throw new Error('WebGPU not supported')
}

function createTextureFromAtlas(
  device: GPUDevice,
  atlas: { data: Uint8Array; atlasWidth: number; atlasHeight: number }
): GPUTexture {
  const bytesPerPixel = 4
  const unpaddedBytesPerRow = atlas.atlasWidth * bytesPerPixel
  const paddedBytesPerRow = Math.ceil(unpaddedBytesPerRow / 256) * 256
  const paddedData = new Uint8Array(paddedBytesPerRow * atlas.atlasHeight)
  for (let row = 0; row < atlas.atlasHeight; row += 1) {
    const srcOffset = row * unpaddedBytesPerRow
    const dstOffset = row * paddedBytesPerRow
    paddedData.set(atlas.data.subarray(srcOffset, srcOffset + unpaddedBytesPerRow), dstOffset)
  }

  const texture = device.createTexture({
    size: [atlas.atlasWidth, atlas.atlasHeight, 1],
    format: 'rgba8unorm',
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  })
  device.queue.writeTexture(
    { texture },
    paddedData,
    { bytesPerRow: paddedBytesPerRow, rowsPerImage: atlas.atlasHeight },
    { width: atlas.atlasWidth, height: atlas.atlasHeight }
  )

  return texture
}

async function init() {
  try {
    const { device, context, format } = await initGpu(canvas)
    device.onuncapturederror = (event) => {
      setStatus(event.error.message)
    }

    const shouldReadback = window.__captureStateVector === true
    const { outputBuffer: initialStateVectorBuffer, readback: stateVectorReadback } = await computeStateVectorSequence(
      device,
      [],
      shouldReadback
    )
    let stateVectorBuffer = initialStateVectorBuffer
    if (stateVectorReadback) {
      window.__stateVector = Array.from(stateVectorReadback)
    }

    const stateTextGlyphBuffer = device.createBuffer({
      size: STATE_TEXT_MAX_LEN * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    })
    device.queue.writeBuffer(stateTextGlyphBuffer, 0, new Uint32Array(STATE_TEXT_MAX_LEN))
    await populateStateTextBuffer(device, stateVectorBuffer, stateTextGlyphBuffer)

    const fontAtlas = createFontAtlas(FONT_GLYPH_SIZE, BASE_GLYPHS)
    const labelGlyphs = buildLabelGlyphs(LABEL_GLYPH_SIZE)
    const labelAtlas = createFontAtlas(LABEL_GLYPH_SIZE, labelGlyphs)

    const fontTexture = createTextureFromAtlas(device, fontAtlas)
    const labelFontTexture = createTextureFromAtlas(device, labelAtlas)
    const fontSampler = device.createSampler({ magFilter: 'nearest', minFilter: 'nearest' })

    const renderer = createRenderer({
      device,
      context,
      format,
      fontTexture,
      labelFontTexture,
      fontAtlasWidth: fontAtlas.atlasWidth,
      fontAtlasHeight: fontAtlas.atlasHeight,
      labelAtlasWidth: labelAtlas.atlasWidth,
      labelAtlasHeight: labelAtlas.atlasHeight,
      fontSampler,
      stateTextGlyphBuffer,
    })

    let stateVectorGlyphCount = 16
    const placedGates: PlacedGate[] = []

    const getGateSequence = () => {
      return [...placedGates]
        .filter((gate) => !gate.dragging)
        .sort((a, b) => (a.x === b.x ? a.id - b.id : a.x - b.x))
        .map((gate) => gate.label)
    }

    const updateScene = () => {
      const scene = buildScene(stateVectorGlyphCount, placedGates)
      const draggingGate = placedGates.find((gate) => gate.dragging) ?? null
      window.__vertexCount = scene.instances.length
      renderer.updateScene(scene, draggingGate)
    }

    const recomputeStateVector = async () => {
      const gates = getGateSequence()
      stateVectorGlyphCount = gates.length === 1 && gates[0] === 'H' ? STATE_TEXT_MAX_LEN : 16
      const result = await computeStateVectorSequence(device, gates, shouldReadback)
      stateVectorBuffer = result.outputBuffer
      if (result.readback) {
        window.__stateVector = Array.from(result.readback)
      }
      await populateStateTextBuffer(device, stateVectorBuffer, stateTextGlyphBuffer)
      updateScene()
    }

    setupInput({
      canvas,
      placedGates,
      onUpdate: updateScene,
      onGateDropped: () => {
        void recomputeStateVector()
      },
    })

    updateScene()

    let hasCaptured = false
    const renderFrame = () => {
      renderer.renderFrame()
      if (!hasCaptured) {
        requestAnimationFrame(() => {
          window.__renderDone = true
        })
        hasCaptured = true
      }
      requestAnimationFrame(renderFrame)
    }
    requestAnimationFrame(renderFrame)
  } catch (error) {
    const message =
      error && typeof (error as { message?: unknown }).message === 'string'
        ? (error as { message: string }).message
        : String(error)
    setStatus(message)
    window.__renderDone = true
    throw error
  }
}

window.__renderDone = false
window.__debugPixel = undefined
if (window.__captureFrame === undefined) {
  window.__captureFrame = false
}
if (window.__captureStateVector === undefined) {
  window.__captureStateVector = false
}
window.__frameDataUrl = undefined
window.__stateVector = undefined
init().catch((error) => {
  console.error(error)
})
