const { registerWorld } = require('./world.ts')
const { registerHooks } = require('./hooks.cjs')

const registerSupport = () => {
  registerWorld()
  registerHooks()
}

registerSupport()

module.exports = {
  registerSupport,
}
