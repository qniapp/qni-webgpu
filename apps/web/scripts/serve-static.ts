import { createStaticServer } from '../test-support/static-server'

const [dir, portText] = process.argv.slice(2)
const port = Number(portText)
if (!dir || !portText || !Number.isInteger(port) || port < 0 || port > 65535) {
  throw new Error('Usage: serve-static.ts <dir> <port>')
}
createStaticServer(dir).listen(port, '127.0.0.1')
