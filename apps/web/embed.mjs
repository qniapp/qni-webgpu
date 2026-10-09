import init, { start_embed } from './qni-web.js'

let initialization

/**
 * Start a local WebGPU editor on a connected canvas, including in a shadow root.
 * @param {HTMLCanvasElement} canvas
 * @param {string} circuit Quirk-style JSON, e.g. '{"cols":[["H"]]}'.
 * @param {{showStatePanel?: boolean}} settings
 * @returns {Promise<{destroy(): void}>}
 */
export async function startEmbed(canvas, circuit, { showStatePanel = true } = {}) {
  if (!initialization) {
    performance.mark('qni:wasm-fetch-start')
    initialization = init({ module_or_path: new URL('./qni-web_bg.wasm', import.meta.url) })
      .then(value => {
        performance.mark('qni:wasm-instantiated')
        performance.measure('qni:wasm-init', 'qni:wasm-fetch-start', 'qni:wasm-instantiated')
        return value
      })
      .catch(error => {
        initialization = undefined
        throw error
      })
  }
  await initialization
  performance.mark('qni:runner-start')
  const runner = await start_embed(canvas, circuit, showStatePanel)
  performance.mark('qni:runner-started')
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
