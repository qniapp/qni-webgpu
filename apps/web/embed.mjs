import init, { start_embed } from './qni-web.js'

let initialization
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
 * @param {{showStatePanel?: boolean, onProgress?: function}} settings
 * @returns {Promise<{destroy(): void}>}
 */
export async function startEmbed(canvas, circuit, { showStatePanel = true, onProgress } = {}) {
  let runner
  try {
    if (onProgress) {
      listeners.add(onProgress)
      onProgress(progress)
    }
    await prepareEmbed()
    if (onProgress) onProgress({ ...progress, stage: 'gpu' })
    performance.mark('qni:runner-start')
    runner = await start_embed(canvas, circuit, showStatePanel)
    performance.mark('qni:runner-started')
    if (onProgress) onProgress({ ...progress, stage: 'prepare' })
  } catch (error) {
    if (runner) {
      runner.destroy()
      runner.free()
    }
    throw error
  } finally {
    if (onProgress) listeners.delete(onProgress)
  }
  let destroyed = false
  return {
    destroy() {
      if (destroyed) return
      destroyed = true
      runner.destroy()
      runner.free()
    },
  }
}
