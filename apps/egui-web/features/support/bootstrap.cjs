const { registerWorld } = require('./world.ts')
const { registerHooks } = require('./hooks.ts')

const registerSupport = () => {
  registerWorld()
  registerHooks()
}

registerSupport()

module.exports = {
  registerSupport,
}
