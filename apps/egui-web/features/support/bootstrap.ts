import { registerHooks } from './hooks'
import { registerWorld } from './world'

export const registerSupport = (): void => {
  registerWorld()
  registerHooks()
}

registerSupport()
