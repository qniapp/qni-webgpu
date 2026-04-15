import init, { read_state_vector, start } from '/qni-egui-web.js'

const statusEl = document.getElementById('app-status')

const hideStatus = () => {
  if (!statusEl) {
    return
  }
  statusEl.hidden = true
  statusEl.textContent = ''
}

const showStatus = (message) => {
  if (!statusEl) {
    return
  }
  statusEl.hidden = false
  statusEl.textContent = message
}

const formatStartupError = (err) => {
  const detail = err && typeof err.message === 'string' ? err.message : String(err)
  return [
    'WebGPU initialization failed.',
    'This browser or environment could not provide a usable WebGPU adapter.',
    detail,
  ].join('\n\n')
}

const run = async () => {
  try {
    await init()
    window.__eguiReadStateVector = async () => {
      try {
        return Array.from(await read_state_vector())
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

run()
