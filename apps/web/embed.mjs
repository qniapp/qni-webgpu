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
  initialization ??= init({ module_or_path: new URL('./qni-web_bg.wasm', import.meta.url) })
    .catch(error => {
      initialization = undefined
      throw error
    })
  await initialization
  const runner = await start_embed(canvas, circuit, showStatePanel)
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
