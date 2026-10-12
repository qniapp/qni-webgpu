import init, { start_embed } from './qni-web.js'

let initialization
let startup = Promise.resolve()
let progress = { stage: 'download', loaded: 0, total: null }
const listeners = new Set()

function publish(update) {
  progress = update
  for (const listener of listeners) listener(update)
}

async function initialize() {
  performance.mark('qni:wasm-fetch-start')
  const response = await fetch(new URL('./qni-web_bg.wasm', import.meta.url))
  if (!response.ok) throw new Error(`Wasm download failed: ${response.status}`)
  // Fetch delivers decoded bytes, whereas Content-Length may describe compressed
  // bytes. Only use a known identity-encoded length for a percentage.
  const length = Number(response.headers.get('content-length'))
  // CORS can hide Content-Encoding even when Content-Length is exposed.
  const total = response.type !== 'cors' && !response.headers.get('content-encoding') && length > 0 ? length : null
  let loaded = 0
  let input = response
  if (response.body) {
    const body = response.body.pipeThrough(new TransformStream({
      transform(chunk, controller) {
        loaded += chunk.byteLength
        publish({ stage: 'download', loaded, total })
        controller.enqueue(chunk)
      },
      flush() {
        performance.mark('qni:wasm-fetch-end')
        publish({ stage: 'compile', loaded, total })
      },
    }))
    // Keep application/wasm so wasm-bindgen uses instantiateStreaming. This is
    // a streaming Response, never a collected ArrayBuffer or buffered clone.
    input = new Response(body, { headers: response.headers })
  }
  const value = await init({ module_or_path: input })
  performance.mark('qni:wasm-instantiated')
  performance.measure('qni:wasm-init', 'qni:wasm-fetch-start', 'qni:wasm-instantiated')
  publish({ stage: 'gpu', loaded, total })
  return value
}

/** Start wasm fetching/streaming compilation before a canvas is connected. */
export function prepareEmbed() {
  if (!initialization) {
    initialization = initialize().catch(error => {
      initialization = undefined
      publish({ stage: 'download', loaded: 0, total: null })
      throw error
    })
  }
  return initialization
}

/**
 * Start a local WebGPU editor on a connected canvas, including in a shadow root.
 * @param {HTMLCanvasElement} canvas
 * @param {string} circuit Quirk-style JSON, e.g. '{"cols":[["H"]]}'.
 * @param {{showStatePanel?: boolean, palette?: string[], maxWireCount?: number, showRestartButton?: boolean, onRestart?: function, onProgress?: function, onDeviceLost?: function}} settings
 *   `palette` lists gate tokens like qni's `mini_qni` filter, e.g. ['|0>', 'H'].
 *   `maxWireCount` is qni's `data-max-wire-count`: the editor adds no empty
 *   wire beyond it, e.g. 1 keeps a one-qubit circuit on one wire while dragging.
 * @returns {Promise<{destroy(): void, circuitJSON(): string, preferredHeight(widthCssPx: number): number, readStateVector(): Promise<Float32Array>}>}
 */
export async function startEmbed(canvas, circuit, { showStatePanel = true, palette, maxWireCount, showRestartButton = false, onRestart, onProgress, onDeviceLost } = {}) {
  let destroyed = false
  let deviceLost = false
  // Defer host actions until egui releases its mutable app borrow. The host
  // may synchronously export the circuit and destroy/recreate this runner.
  const restart = showRestartButton && onRestart ? () => queueMicrotask(() => {
    if (!destroyed && !deviceLost) onRestart()
  }) : undefined
  let runner
  const lost = () => { deviceLost = true; onDeviceLost?.() }
  globalThis.addEventListener?.('qni-device-lost', lost)
  try {
    if (onProgress) {
      listeners.add(onProgress)
      onProgress(progress)
    }
    await prepareEmbed()
    if (onProgress) onProgress({ ...progress, stage: 'gpu' })
    performance.mark('qni:runner-start')
    const starting = startup.then(() => start_embed(canvas, circuit, showStatePanel, palette, maxWireCount, showRestartButton, restart))
    startup = starting.catch(() => {})
    runner = await starting
    if (deviceLost) throw new Error('Qni GPU device lost during startup')
    performance.mark('qni:runner-started')
    if (onProgress) onProgress({ ...progress, stage: 'prepare' })
  } catch (error) {
    globalThis.removeEventListener?.('qni-device-lost', lost)
    if (runner) {
      runner.destroy()
      runner.free()
    }
    throw error
  } finally {
    if (onProgress) listeners.delete(onProgress)
  }
  return {
    circuitJSON() { return runner.circuit_json() },
    preferredHeight(widthCssPx) { return runner.preferred_height(widthCssPx) },
    // Test-only, explicitly requested GPU readback, never part of rendering.
    readStateVector() { return runner.read_state_vector() },
    destroy() {
      if (destroyed) return
      destroyed = true
      globalThis.removeEventListener?.('qni-device-lost', lost)
      runner.destroy()
      runner.free()
    },
  }
}
