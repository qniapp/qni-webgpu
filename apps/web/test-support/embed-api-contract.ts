import type { Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import path from 'node:path'

export type EmbedSettings = {
  showStatePanel?: boolean
  palette?: string[]
  maxWireCount?: number
  showRestartButton?: boolean
}

/** Real asset origin: route interception is deliberately limited to the host HTML. */
export class EmbedContractHost {
  private static server: Server | undefined
  static assetURL: string

  static async listen() {
    const prefix = '/vendor/qni/abc/'
    const root = path.resolve(__dirname, '../embed-dist')
    const server = createServer(async (request, response) => {
      const pathname = new URL(request.url!, 'http://localhost').pathname
      const file = pathname.slice(prefix.length)
      if (!pathname.startsWith(prefix) || !/^[\w.-]+$/.test(file)) {
        response.writeHead(404).end()
        return
      }
      try {
        const body = await readFile(path.join(root, file))
        response.writeHead(200, {
          'Access-Control-Allow-Origin': '*',
          'Content-Type': file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript',
          'Content-Length': body.length,
          'Cache-Control': 'no-store',
        }).end(body)
      } catch {
        response.writeHead(404).end()
      }
    })
    this.server = server
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject)
      server.listen(0, '127.0.0.1', resolve)
    })
    const address = server.address()
    if (!address || typeof address === 'string') throw new Error('Missing asset server address')
    this.assetURL = `http://127.0.0.1:${address.port}${prefix}`
  }

  static async close() {
    const server = this.server
    this.server = undefined
    if (!server) return
    await new Promise<void>((resolve, reject) => {
      server.close(error => error ? reject(error) : resolve())
      server.closeIdleConnections()
    })
  }

  static async open(page: Page) {
    await page.route('**/host/embed-contract', route => route.fulfill({
      contentType: 'text/html',
      body: `<!doctype html><html><head><meta charset="utf-8"></head>
        <body style="margin:0"><script>
        window.unhandled = [];
        addEventListener('unhandledrejection', event => window.unhandled.push(String(event.reason)));
        </script></body></html>`,
    }))
    await page.goto('/host/embed-contract')
    // Routed HTML has no network address-space classification. Current Chrome
    // therefore needs explicit permission to reach the real loopback listener.
    await page.context().grantPermissions(['local-network-access'], { origin: new URL(page.url()).origin })
    await page.evaluate(async assetURL => {
      const w = window as any
      w.api = await import(`${assetURL}qni-embed.mjs`)
      w.runners = []
      w.canvases = []
      w.progress = []
      w.losses = []
      w.restarts = []
      w.start = async (circuit: string, settings: object = {}) => {
        const index = w.canvases.length
        const host = document.createElement('qni-contract-test')
        const shadow = host.attachShadow({ mode: 'open' })
        shadow.innerHTML = '<style>:host{display:block;width:960px;height:640px}canvas{display:block;width:100%;height:100%}</style><canvas tabindex="0"></canvas>'
        document.body.append(host)
        const canvas = shadow.querySelector('canvas')!
        w.canvases.push(canvas)
        const runner = await w.api.startEmbed(canvas, circuit, {
          ...settings,
          onProgress: (update: unknown) => w.progress.push({ index, ...update as object }),
          onDeviceLost: (error: any) => w.losses.push({ index, code: error.code }),
          onRestart: () => w.restarts.push({ index, circuit: w.runners[index].circuitJSON() }),
        })
        w.runners[index] = runner
        return index
      }
    }, this.assetURL)
  }

  static async start(page: Page, circuit = '{"cols":[["H"]]}', settings: EmbedSettings = {}) {
    return page.evaluate(async ({ circuit, settings }) => (window as any).start(circuit, settings), { circuit, settings })
  }

  static canvas(page: Page) { return page.locator('qni-contract-test canvas').first() }

  static async frames(page: Page) {
    await page.evaluate(async () => {
      for (let i = 0; i < 4; i++) await new Promise(requestAnimationFrame)
    })
  }

  static async drop(page: Page, fromX: number, toX: number, toY = 216) {
    const box = (await this.canvas(page).boundingBox())!
    await page.mouse.move(box.x + fromX, box.y + 100)
    await page.mouse.down()
    await page.mouse.move(box.x + 300, box.y + 180, { steps: 8 })
    await page.mouse.move(box.x + toX, box.y + toY, { steps: 8 })
    await page.mouse.up()
    await page.mouse.move(box.x + 900, box.y + 100)
  }

  static async waitForCircuit(page: Page, circuit: string) {
    await page.waitForFunction(circuit => (window as any).runners[0].circuitJSON() === circuit, circuit)
  }
}
