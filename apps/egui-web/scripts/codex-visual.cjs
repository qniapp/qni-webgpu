#!/usr/bin/env node
const path = require('node:path')
const { chromium } = require('playwright')

const { getCodexVisualLaunchOptions } = require('../test-support/browser-launch.cjs')
const {
  buildDragOperation,
  buildScreenshotPlan,
  parseOperations,
} = require('../test-support/codex-visual-command.cjs')
const { getPlaywrightBaseUrl } = require('../test-support/web-server.cjs')
const {
  dragPointer,
  readStateVector,
  waitForCanvasContent,
  waitForStartupReady,
} = require('../features/support/egui-helpers.cjs')

const usage = () => {
  console.error(`Usage:
  node scripts/codex-visual.cjs screenshot [--url URL] [--out PATH]
  node scripts/codex-visual.cjs drag --gate H --wire q0 --slot 0 [--url URL] [--out PATH]
  node scripts/codex-visual.cjs ops --ops H:q0:0,C:q0:1,X:q1:1 [--url URL] [--out PATH]

Environment:
  HEADLESS=1 hides the browser. QNI_EGUI_WEB_EXTERNAL_SERVER=1 uses the existing server.`)
}

const parseArgs = (argv) => {
  const [command, ...rest] = argv
  const options = { command }

  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index]
    if (!arg.startsWith('--')) {
      throw new Error(`Unexpected argument: ${arg}`)
    }
    const key = arg.slice(2)
    const value = rest[index + 1]
    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for --${key}`)
    }
    options[key] = value
    index += 1
  }

  return options
}

const placeGate = async (page, operationInput) => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('Could not find #egui-canvas bounding box')
  }

  const operation = buildDragOperation({ ...operationInput, cssWidth: box.width })
  await dragPointer(page, operation.from, operation.to, 12)
  return operation
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (!options.command || options.help) {
    usage()
    process.exit(options.command ? 0 : 1)
  }

  const launchOptions = getCodexVisualLaunchOptions({
    env: process.env,
    defaultPath: chromium.executablePath(),
  })
  const browser = await chromium.launch(launchOptions)

  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } })
    await page.goto(options.url || getPlaywrightBaseUrl({ env: process.env }))
    await waitForStartupReady(page, { waitForStateVector: true })

    const operations = []
    if (options.command === 'drag') {
      operations.push(await placeGate(page, {
        gate: options.gate,
        wire: options.wire,
        slot: options.slot,
        verticalOffset: Number.parseFloat(options['vertical-offset'] ?? '8'),
      }))
    } else if (options.command === 'ops') {
      for (const operation of parseOperations(options.ops)) {
        operations.push(await placeGate(page, {
          ...operation,
          verticalOffset: Number.parseFloat(options['vertical-offset'] ?? '8'),
        }))
      }
    } else if (options.command !== 'screenshot') {
      throw new Error(`Unknown command: ${options.command}`)
    }

    const canvas = page.locator('#egui-canvas')
    const screenshotPlan = buildScreenshotPlan({
      command: options.command,
      out: options.out,
      canvasOut: options['canvas-out'],
    })
    const render = await waitForCanvasContent(
      page,
      canvas,
      screenshotPlan.canvasOut ? { path: screenshotPlan.canvasOut } : {}
    )
    await page.screenshot({ path: screenshotPlan.pageOut, fullPage: true })
    const stateVector = await readStateVector(page)

    console.log(JSON.stringify({
      url: page.url(),
      screenshot: path.resolve(screenshotPlan.pageOut),
      canvasScreenshot: screenshotPlan.canvasOut ? path.resolve(screenshotPlan.canvasOut) : null,
      nonBackground: render.nonBackground,
      stateVector,
      operations,
    }, null, 2))
  } finally {
    await browser.close()
  }
}

main().catch((error) => {
  console.error(error.message)
  usage()
  process.exit(1)
})
