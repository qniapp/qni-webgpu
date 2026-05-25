type QniWebModule = {
  default: () => Promise<void>
  circuit_library_clear: () => void
  circuit_library_delete: (id: string) => void
  circuit_library_list: () => string
  circuit_library_load: (id: string) => string
  circuit_library_rename: (id: string, name: string) => void
  circuit_library_save: (name: string, circuitJson: string) => string
  read_state_vector: () => Promise<ArrayLike<number>>
  read_bloch_vectors: () => Promise<ArrayLike<number>>
  read_amplitude_cell: (gateId: number, outcome: number) => Promise<ArrayLike<number>>
  read_density_matrix_cell: (gateId: number, row: number, col: number) => Promise<ArrayLike<number>>
  read_probability_distributions: () => Promise<ArrayLike<number>>
  read_measurement_outcomes: () => Promise<ArrayLike<number>>
  start: (canvasId: string) => Promise<void>
}

declare global {
  interface Window {
    __eguiError?: unknown
    __eguiReady?: boolean
    __eguiReadStateVector?: () => unknown[] | Promise<unknown[]>
    __eguiReadBlochVectors?: () => Promise<number[]>
    __eguiReadProbabilityDistributions?: () => Promise<number[]>
    __eguiReadAmplitudeCell?: (gateId: number, outcome: number) => Promise<number[]>
    __eguiReadDensityMatrixCell?: (gateId: number, row: number, col: number) => Promise<number[]>
    __eguiReadMeasurementOutcomes?: () => Promise<number[]>
    __qniExecModeFocusRequested?: boolean
    __qniQiskitBackendUrl?: string
    __qniLastQiskitRequest?: unknown
    __qniLastQiskitResult?: unknown
    __qniRunQiskitBackend?: (payloadJson: string) => Promise<unknown>
    __qniCircuitLibraryClear?: () => void
    __qniCircuitLibraryDelete?: (id: string) => void
    __qniCircuitLibraryList?: () => string
    __qniCircuitLibraryLoad?: (id: string) => string
    __qniCircuitLibraryRename?: (id: string, name: string) => void
    __qniCircuitLibrarySave?: (name: string, circuitJson: string) => string
    __setExternalGpuStatus?: (json: string | unknown) => void
  }
}

type QniBackendError = Error & { qniHttpStatus?: number }

const bootstrapScriptUrl = (): string => {
  const script = Array.from(document.scripts).find((element) => element.src.endsWith('/bootstrap.js'))
  return script?.src ?? new URL('bootstrap.js', window.location.href).toString()
}

const wasmModulePath = new URL('qni-web.js', bootstrapScriptUrl()).toString()
const loadQniWeb = async (): Promise<QniWebModule> => import(wasmModulePath) as Promise<QniWebModule>

const defaultQiskitBackendUrl = (): string => {
  if (window.location.port === '4174' && ['127.0.0.1', 'localhost'].includes(window.location.hostname)) {
    return 'http://127.0.0.1:4184/run'
  }
  return new URL('run', window.location.href).toString()
}

const qiskitBackendUrl = (): string => window.__qniQiskitBackendUrl ?? defaultQiskitBackendUrl()

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
      'The browser could not fetch /qni-web.js — usually a stale',
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
      circuit_library_clear,
      circuit_library_delete,
      circuit_library_list,
      circuit_library_load,
      circuit_library_rename,
      circuit_library_save,
      read_amplitude_cell,
      read_density_matrix_cell,
      read_bloch_vectors,
      read_probability_distributions,
      read_measurement_outcomes,
      read_state_vector,
      start,
    } = await loadQniWeb()
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
    window.__eguiReadProbabilityDistributions = async () => {
      try {
        return Array.from(await read_probability_distributions())
      } catch {
        return []
      }
    }
    window.__eguiReadAmplitudeCell = async (gateId: number, outcome: number) => {
      try {
        return Array.from(await read_amplitude_cell(gateId, outcome))
      } catch {
        return []
      }
    }
    window.__eguiReadDensityMatrixCell = async (gateId: number, row: number, col: number) => {
      try {
        return Array.from(await read_density_matrix_cell(gateId, row, col))
      } catch {
        return []
      }
    }
    window.__qniCircuitLibraryClear = circuit_library_clear
    window.__qniCircuitLibraryDelete = circuit_library_delete
    window.__qniCircuitLibraryList = circuit_library_list
    window.__qniCircuitLibraryLoad = circuit_library_load
    window.__qniCircuitLibraryRename = circuit_library_rename
    window.__qniCircuitLibrarySave = circuit_library_save
    const canvas = document.getElementById('egui-canvas') as HTMLCanvasElement | null
    if (canvas) {
      canvas.tabIndex = 0
    }
    window.__qniExecModeFocusRequested = false
    window.__qniRunQiskitBackend = async (payloadJson: string): Promise<unknown> => {
      const payload = JSON.parse(payloadJson) as unknown
      window.__qniLastQiskitRequest = payload
      window.__qniLastQiskitResult = undefined
      const response = await fetch(qiskitBackendUrl(), {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(payload),
      })
      const body = await response.json().catch((): unknown => undefined)
      if (!response.ok) {
        const detail = typeof body === 'object' && body !== null && 'message' in body
          ? String((body as { message?: unknown }).message)
          : `HTTP ${response.status}`
        const error: QniBackendError = new Error(detail)
        error.qniHttpStatus = response.status
        throw error
      }
      window.__qniLastQiskitResult = body
      return body
    }
    document.addEventListener('keydown', (event) => {
      if (event.key !== 'Tab' || event.shiftKey || event.defaultPrevented) {
        return
      }
      window.__qniExecModeFocusRequested = true
      canvas?.focus()
      event.preventDefault()
    }, { capture: true })
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
