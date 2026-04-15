const { test, expect } = require('@playwright/test')
const { chromium } = require('playwright')

const evaluateWithRetry = async (page, fn, arg, attempts = 3) => {
  let lastError
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await page.evaluate(fn, arg)
    } catch (error) {
      lastError = error
      if (!String(error).includes('Execution context was destroyed')) {
        throw error
      }
      await page.waitForLoadState('load')
      await page.waitForFunction(
        () => window.__eguiReady === true || Boolean(window.__eguiError),
        null,
        { timeout: 20000 }
      )
    }
  }
  throw lastError
}

const screenshotWithRetry = async (page, locator, options, attempts = 3) => {
  let lastError
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await locator.screenshot(options)
    } catch (error) {
      lastError = error
      const message = String(error)
      if (
        !message.includes('Execution context was destroyed') &&
        !message.includes('Element is not attached') &&
        !message.includes('Cannot find context')
      ) {
        throw error
      }
      await page.waitForLoadState('load')
      await page.waitForFunction(
        () => window.__eguiReady === true || Boolean(window.__eguiError),
        null,
        { timeout: 20000 }
      )
      await waitForStateVectorReady(page)
    }
  }
  throw lastError
}

const readStateVector = async (page) =>
  evaluateWithRetry(page, async () => {
    if (!window.__eguiReadStateVector) {
      return []
    }
    return await window.__eguiReadStateVector()
  })

const waitForStateVectorReady = async (page, timeout = 20000) => {
  await expect
    .poll(async () => (await readStateVector(page)).length > 0, { timeout })
    .toBe(true)
}

const waitForStateVectorLength = async (page, length, timeout = 5000) => {
  await expect
    .poll(async () => (await readStateVector(page)).length, { timeout })
    .toBe(length)
}

const waitForStateVectorApprox = async (page, expected, timeout = 5000, tolerance = 1e-3) => {
  await expect
    .poll(async () => {
      const actual = await readStateVector(page)
      if (actual.length !== expected.length) {
        return false
      }
      return expected.every((value, index) => Math.abs(actual[index] - value) < tolerance)
    }, { timeout })
    .toBe(true)
}

const waitForCanvasBlue = async (page, canvas, path, timeout = 5000) => {
  const start = Date.now()
  let lastHits = 0
  let lastScreenshot
  while (Date.now() - start < timeout) {
    lastScreenshot = await screenshotWithRetry(page, canvas, { type: 'png', path })
    const base64 = lastScreenshot.toString('base64')
    const { hits } = await evaluateWithRetry(
      page,
      async ({ base64 }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })

        const canvas = document.createElement('canvas')
        canvas.width = img.width
        canvas.height = img.height
        const ctx = canvas.getContext('2d', { willReadFrequently: true })
        if (!ctx) {
          return { hits: 0 }
        }
        ctx.drawImage(img, 0, 0)
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
        const data = imageData.data

        let hits = 0
        for (let y = Math.floor(canvas.height * 0.6); y < canvas.height; y += 2) {
          for (let x = 0; x < canvas.width; x += 2) {
            const idx = (y * canvas.width + x) * 4
            const r = data[idx]
            const g = data[idx + 1]
            const b = data[idx + 2]
            if (b > r + 30 && b > g + 30) {
              hits += 1
            }
          }
        }
        return { hits }
      },
      { base64 }
    )
    lastHits = hits
    if (hits > 100) {
      return { hits, screenshot: lastScreenshot }
    }
    await page.waitForTimeout(100)
  }
  return { hits: lastHits, screenshot: lastScreenshot }
}

const dragPointer = async (page, from, to, steps = 6, release = true) => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  const startX = box.x + from.x
  const startY = box.y + from.y
  const endX = box.x + to.x
  const endY = box.y + to.y
  await page.mouse.move(startX, startY)
  await page.mouse.down()
  await page.waitForTimeout(16)
  await page.mouse.move(endX, endY, { steps })
  await page.waitForTimeout(16)
  if (release) {
    await page.mouse.up()
  }
}

const releasePointer = async (page, at) => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  const endX = box.x + at.x
  const endY = box.y + at.y
  await page.mouse.move(endX, endY)
  await page.mouse.up()
}

test('egui webgpu canvas renders content', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  const gpuAvailable = await page.evaluate(() => Boolean(navigator.gpu))
  expect(gpuAvailable).toBe(true)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)
  const cssHeight = box?.height ?? (viewport?.height ?? 800)

  await waitForStateVectorReady(page)
  const initialState = await readStateVector(page)
  expect(initialState).toEqual([1, 0, 0, 0])
  const stateCount = Math.max(1, initialState.length / 2)

  await page.waitForTimeout(500)
  const { hits: initialHits } = await waitForCanvasBlue(
    page,
    canvas,
    '/tmp/qni-egui-webgpu-initial.png'
  )
  expect(initialHits).toBeGreaterThan(100)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2
  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y
  const targetX2 = targetX + SLOT_SPACING
  const targetY2 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY })

  const expected = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY })

  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX2, y: targetY2 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)

  const screenshot = await screenshotWithRetry(page, canvas, {
    type: 'png',
    path: '/tmp/qni-egui-webgpu-after.png'
  })
  const base64 = screenshot.toString('base64')

  const stats = await evaluateWithRetry(page, async ({ base64 }) => {
    const img = new Image()
    img.src = `data:image/png;base64,${base64}`
    await new Promise((resolve, reject) => {
      img.onload = () => resolve(null)
      img.onerror = () => reject(new Error('Failed to decode screenshot'))
    })

    const canvas = document.createElement('canvas')
    canvas.width = img.width
    canvas.height = img.height
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) {
      return { nonBackground: 0 }
    }
    ctx.drawImage(img, 0, 0)
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
    const data = imageData.data
    const width = imageData.width
    const height = imageData.height
    const background = [255, 255, 255]
    const threshold = 20
    let nonBackground = 0

    for (let y = 0; y < height; y += 4) {
      for (let x = 0; x < width; x += 4) {
        const idx = (y * width + x) * 4
        const diff =
          Math.abs(data[idx] - background[0]) +
          Math.abs(data[idx + 1] - background[1]) +
          Math.abs(data[idx + 2] - background[2])
        if (diff > threshold) {
          nonBackground += 1
        }
      }
    }

    return { nonBackground }
  }, { base64 })

  expect(stats.nonBackground).toBeGreaterThan(40)
  await canvas.screenshot({ path: '/tmp/qni-egui-webgpu.png' })
})

test('H on q0 and q1 yields uniform superposition', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  const expectedAfterQ0 = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedAfterQ0)

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })

  const expected = [0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0]
  await waitForStateVectorApprox(page, expected)
})

test('dragging does not grow state vector until drop', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  const targetY2 = LINE_Y + 2 * LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })
  await waitForStateVectorLength(page, 8)

  await dragPointer(page, { x: targetX, y: targetY0 }, { x: targetX, y: targetY2 }, 6, false)

  const lengthDuringDrag = (await readStateVector(page)).length
  expect(lengthDuringDrag).toBe(8)

  await releasePointer(page, { x: targetX, y: targetY2 })

  await waitForStateVectorLength(page, 16)
})

test('CNOT with control on q1 yields bell state', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })

  const expectedAfterH = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0, 0, 0, 0, 0]
  await waitForStateVectorApprox(page, expectedAfterH)

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY1 })

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX2, y: targetY0 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)
})

test('Control does not affect gates in other columns', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetX3 = targetX2 + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY0 })

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX3, y: targetY1 })

  const expected = [0, 0, 1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)
})

test('default chromium shows a visible WebGPU error instead of a blank page', async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH || chromium.executablePath(),
    args: ['--disable-gpu', '--disable-software-rasterizer'],
  })

  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } })
    await page.goto('http://127.0.0.1:4174/', { waitUntil: 'load' })
    await page.waitForFunction(
      () => window.__eguiReady === true || Boolean(window.__eguiError),
      null,
      { timeout: 20000 }
    )

    await expect.poll(async () => page.evaluate(() => window.__eguiError || null), {
      timeout: 20000,
    }).not.toBeNull()
    await expect(page.locator('[data-testid="webgpu-error"]')).toBeVisible()
    await expect(page.locator('[data-testid="webgpu-error"]')).toContainText('WebGPU')
  } finally {
    await browser.close()
  }
})
