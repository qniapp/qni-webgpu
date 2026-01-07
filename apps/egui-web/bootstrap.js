import init, { read_state_vector, start } from '/qni-egui-web.js'

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
    promise.catch((err) => {
      window.__eguiError = String(err)
      console.error(err)
    })
  } catch (err) {
    window.__eguiError = String(err)
    console.error(err)
  }
}

run()
