const { registerWorld } = require('./world.cjs')
const { registerHooks } = require('./hooks.cjs')

const registerSupport = () => {
  registerWorld()
  registerHooks()
}

registerSupport()

module.exports = {
  registerSupport,
}
