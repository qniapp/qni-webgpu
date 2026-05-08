type QniEguiWebModule = {
  default: () => Promise<void>
  read_state_vector: () => Promise<ArrayLike<number>>
  read_bloch_vectors: () => ArrayLike<number>
  start: (canvasId: string) => Promise<void>
}

declare global {
  interface Window {
    __eguiError?: unknown
    __eguiReady?: boolean
    __eguiReadStateVector?: () => unknown[] | Promise<unknown[]>
    __eguiReadBlochVectors?: () => number[]
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
  return [
    'WebGPU initialization failed.',
    'This browser or environment could not provide a usable WebGPU adapter.',
    detail,
  ].join('\n\n')
}

const run = async (): Promise<void> => {
  try {
    const { default: init, read_bloch_vectors, read_state_vector, start } = await loadQniEguiWeb()
    await init()
    window.__eguiReadStateVector = async () => {
      try {
        return Array.from(await read_state_vector())
      } catch {
        return []
      }
    }
    window.__eguiReadBlochVectors = () => {
      try {
        return Array.from(read_bloch_vectors())
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
