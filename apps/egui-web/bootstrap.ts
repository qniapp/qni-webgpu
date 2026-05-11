type QniEguiWebModule = {
  default: () => Promise<void>
  read_state_vector: () => Promise<ArrayLike<number>>
  read_bloch_vectors: () => Promise<ArrayLike<number>>
  read_measurement_outcomes: () => Promise<ArrayLike<number>>
  start: (canvasId: string) => Promise<void>
}

declare global {
  interface Window {
    __eguiError?: unknown
    __eguiReady?: boolean
    __eguiReadStateVector?: () => unknown[] | Promise<unknown[]>
    __eguiReadBlochVectors?: () => Promise<number[]>
    __eguiReadMeasurementOutcomes?: () => Promise<number[]>
  }
}

const wasmModulePath = '/qni-egui-web.js'
const loadQniEguiWeb = async (): Promise<QniEguiWebModule> =>
  import(wasmModulePath) as Promise<QniEguiWebModule>

const statusEl = document.getElementById('app-status')

const hideStatus = (): void => {
  if (!statusEl) {
    return
  }
  statusEl.hidden = true
  statusEl.textContent = ''
}

const showStatus = (message: string): void => {
  if (!statusEl) {
    return
  }
  statusEl.hidden = false
  statusEl.textContent = message
}

const formatStartupError = (err: unknown): string => {
  const detail = err instanceof Error ? err.message : String(err)
  // The dynamic `import()` of the wasm-bindgen JS shim throws a
  // TypeError with the literal "Failed to fetch dynamically imported
  // module" message when the browser cache holds a stale reference to
  // an asset trunk has since replaced. That's a totally different
  // problem from a missing WebGPU adapter, so peel the two cases apart.
  if (detail.includes('Failed to fetch dynamically imported module')) {
    return [
      'Asset load failed.',
      'The browser could not fetch /qni-egui-web.js — usually a stale',
      'cache from a previous dev build. Hard reload (Ctrl+Shift+R) to',
      'force a fresh download.',
      detail,
    ].join('\n\n')
  }
  return [
    'WebGPU initialization failed.',
    'This browser or environment could not provide a usable WebGPU adapter.',
    detail,
  ].join('\n\n')
}

const run = async (): Promise<void> => {
  try {
    const {
      default: init,
      read_bloch_vectors,
      read_measurement_outcomes,
      read_state_vector,
      start,
    } = await loadQniEguiWeb()
    await init()
    window.__eguiReadStateVector = async () => {
      try {
        return Array.from(await read_state_vector())
      } catch {
        return []
      }
    }
    window.__eguiReadBlochVectors = async () => {
      try {
        return Array.from(await read_bloch_vectors())
      } catch {
        return []
      }
    }
    window.__eguiReadMeasurementOutcomes = async () => {
      try {
        return Array.from(await read_measurement_outcomes())
      } catch {
        return []
      }
    }
    const promise = start('egui-canvas')
    window.__eguiReady = true
    promise
      .then(() => {
        hideStatus()
      })
      .catch((err) => {
        window.__eguiError = String(err)
        showStatus(formatStartupError(err))
        console.error(err)
      })
  } catch (err) {
    window.__eguiError = String(err)
    showStatus(formatStartupError(err))
    console.error(err)
  }
}

void run()

export {}
