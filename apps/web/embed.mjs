import { QniStartupError, classifyStartupError, preflightSync, diagnoseWebGpu } from './qni-embed-error.mjs'
export { QniStartupError, classifyStartupError, preflightSync, diagnoseWebGpu, STARTUP_ERROR_CODES, renderStartupError, detectPlatform, startupErrorContent } from './qni-embed-error.mjs'

export const EMBED_API_VERSION = 1
const buildSourceCommit = '__QNI_SOURCE_COMMIT__'
export const SOURCE_COMMIT = /^[0-9a-f]{40}$/i.test(buildSourceCommit) ? buildSourceCommit : 'unknown'

let start_embed

let initialization
let progress = { stage: 'download', loaded: 0, total: null }
const listeners = new Set()

function publish(update) {
  progress = update
  for (const listener of listeners) listener(update)
}

async function initialize() {
  performance.mark('qni:wasm-fetch-start')
  let response
  try {
    response = await fetch(new URL('./qni-web_bg.wasm', import.meta.url))
  } catch (error) {
    throw new QniStartupError('load-failed', `Wasm download failed: ${error?.message ?? String(error)}`, {
      cause: error, detail: { offline: globalThis.navigator?.onLine === false },
    })
  }
  if (!response.ok) throw new QniStartupError('load-failed', `Wasm download failed: ${response.status}`, { detail: { status: response.status } })
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
  let value
  try {
    const wasm = await import('./qni-web.js')
    start_embed = wasm.start_embed
    value = await wasm.default({ module_or_path: input })
  } catch (error) {
    if (error instanceof WebAssembly.CompileError || error instanceof WebAssembly.LinkError) {
      throw new QniStartupError('load-failed', `Wasm compile failed: ${error.message}`, { cause: error })
    }
    throw classifyStartupError(error)
  }
  performance.mark('qni:wasm-instantiated')
  performance.measure('qni:wasm-init', 'qni:wasm-fetch-start', 'qni:wasm-instantiated')
  publish({ stage: 'gpu', loaded, total })
  return value
}

/** Start wasm fetching/streaming compilation before a canvas is connected. */
export function prepareEmbed() {
  const early = preflightSync()
  if (early) return Promise.reject(early)
  if (!initialization) {
    initialization = initialize().catch(error => {
      initialization = undefined
      publish({ stage: 'download', loaded: 0, total: null })
      throw classifyStartupError(error)
    })
  }
  return initialization
}

// Probe and runner waits have independent GPU deadlines; downloading is never timed.
function startWithTimeout(start, timeoutMs, dispose = runner => {
  try { runner.destroy() } finally { runner.free() }
}) {
  if (!(timeoutMs > 0)) return start()
  return new Promise((resolve, reject) => {
    let expired = false
    const timer = setTimeout(() => {
      expired = true
      reject(new QniStartupError('startup-timeout', `WebGPU initialization did not finish within ${timeoutMs} ms (stage: gpu)`, {
        detail: { timeoutMs, stage: 'gpu' },
      }))
    }, timeoutMs)
    Promise.resolve().then(start).then(runner => {
      if (expired) {
        dispose(runner)
      } else {
        clearTimeout(timer)
        resolve(runner)
      }
    }, error => {
      clearTimeout(timer)
      reject(error)
    }).catch(() => {}) // Late cleanup must not produce an unhandled rejection.
  })
}

/**
 * Start a local WebGPU editor on a connected canvas, including in a shadow root.
 * @param {HTMLCanvasElement} canvas
 * @param {string} circuit Quirk-style JSON, e.g. '{"cols":[["H"]]}'.
 * @param {{showStatePanel?: boolean, palette?: string[], maxWireCount?: number, showRestartButton?: boolean, onRestart?: function, onProgress?: function, onDeviceLost?: function, startupTimeoutMs?: number}} settings
 *   `palette` lists gate tokens like qni's `mini_qni` filter, e.g. ['|0>', 'H'].
 *   `maxWireCount` is qni's `data-max-wire-count`: the editor adds no empty
 *   wire beyond it, e.g. 1 keeps a one-qubit circuit on one wire while dragging.
 * @returns {Promise<{destroy(): void, circuitJSON(): string, preferredHeight(widthCssPx: number): number, selectStep(n: number): void, selectedStep(): number | null, stepCount(): number, readStateVector(): Promise<Float32Array>}>}
 */
export async function startEmbed(canvas, circuit, { showStatePanel = true, palette, maxWireCount, showRestartButton = false, onRestart, onProgress, onDeviceLost, startupTimeoutMs = 20_000 } = {}) {
  let destroyed = false
  let deviceLost = false
  // Defer host actions until egui releases its mutable app borrow. The host
  // may synchronously export the circuit and destroy/recreate this runner.
  const restart = showRestartButton && onRestart ? () => queueMicrotask(() => {
    if (!destroyed && !deviceLost) onRestart()
  }) : undefined
  let runner
  let lostError
  const lost = event => {
    if (destroyed || deviceLost) return
    deviceLost = true
    lostError = new QniStartupError('gpu-device-lost', runner ? 'Qni GPU device lost' : 'Qni GPU device lost during startup', { detail: event?.detail ?? {} })
    onDeviceLost?.(lostError)
  }
  globalThis.addEventListener?.('qni-device-lost', lost)
  try {
    if (onProgress) {
      listeners.add(onProgress)
      onProgress(progress)
    }
    const early = preflightSync()
    if (early) throw early
    const gpuCheck = startWithTimeout(() => diagnoseWebGpu(), startupTimeoutMs, () => {})
    const wasm = prepareEmbed()
    wasm.catch(() => {})
    const gpuProblem = await gpuCheck
    if (gpuProblem) throw gpuProblem
    await wasm
    if (onProgress) onProgress({ ...progress, stage: 'gpu' })
    if (deviceLost) throw lostError
    if (canvas.isConnected === false) throw new QniStartupError('invalid-embed', 'Embed canvas is disconnected')
    performance.mark('qni:runner-start')
    runner = await startWithTimeout(() => start_embed(canvas, circuit, showStatePanel, palette, maxWireCount, showRestartButton, restart), startupTimeoutMs)
    if (deviceLost) throw lostError
    if (canvas.isConnected === false) throw new QniStartupError('invalid-embed', 'Embed canvas is disconnected')
    performance.mark('qni:runner-started')
    if (onProgress) onProgress({ ...progress, stage: 'prepare' })
  } catch (error) {
    globalThis.removeEventListener?.('qni-device-lost', lost)
    if (runner) {
      runner.destroy()
      runner.free()
    }
    throw classifyStartupError(error)
  } finally {
    if (onProgress) listeners.delete(onProgress)
  }
  const requireRunner = (allowDeviceLost = false) => {
    if (destroyed) throw Object.assign(new Error('Circuit runner is destroyed'), { code: 'runner-destroyed' })
    if (deviceLost && !allowDeviceLost) throw Object.assign(new Error('Qni GPU device lost'), { code: 'gpu-device-lost' })
  }
  return {
    circuitJSON() { requireRunner(true); return runner.circuit_json() },
    preferredHeight(widthCssPx) { requireRunner(); return runner.preferred_height(widthCssPx) },
    selectStep(n) { requireRunner(); return runner.selectStep(n) },
    selectedStep() { requireRunner(); return runner.selectedStep() ?? null },
    stepCount() { requireRunner(); return runner.stepCount() },
    // Explicitly requested GPU readback, never part of rendering.
    async readStateVector() { requireRunner(); return runner.read_state_vector() },
    destroy() {
      if (destroyed) return
      destroyed = true
      globalThis.removeEventListener?.('qni-device-lost', lost)
      runner.destroy()
      runner.free()
    },
  }
}
